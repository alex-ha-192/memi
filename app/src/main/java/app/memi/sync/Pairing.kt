package app.memi.sync

import org.json.JSONObject
import java.nio.ByteBuffer
import java.security.MessageDigest
import java.security.KeyFactory
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.PrivateKey
import java.security.PublicKey
import java.security.SecureRandom
import java.security.spec.ECGenParameterSpec
import java.security.spec.X509EncodedKeySpec
import java.util.Base64
import javax.crypto.Cipher
import javax.crypto.KeyAgreement
import javax.crypto.Mac
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Hands the sync key from your phone to a new computer by QR code, without it ever being readable
 * on the way.
 *
 * 1. The computer makes a one-off key pair and shows its public half in a QR code, with a pairing id.
 * 2. The phone scans it, makes its own one-off key pair, and agrees a shared secret with the
 *    computer's public key (ECDH on P-256, then HKDF-SHA256). It encrypts the sync key with that
 *    secret (AES-GCM) and drops the result in your Drive app folder as memi-pair-<id>.bin.
 * 3. The computer picks the file up, decrypts it with its private half, checks that the key really
 *    opens your backup, and deletes the file.
 *
 * Only the computer that showed the code can open the file, and the code is useless once used.
 *
 * It works the other way round too ("start anywhere"): a phone without the key makes an [Offer]
 * and leaves it in Drive as a join request (memi-join-<id>.json), and a computer, the web app or
 * another phone that has the key answers it. Both sides show the same 6-digit [confirmCode].
 * Byte-for-byte compatible with core/src/pairing.rs.
 */
object Pairing {
    private const val PREFIX = "memi:pair:1:"
    const val JOIN_PREFIX = "memi-join-"
    /** Join requests older than this are ignored and cleaned up. */
    const val JOIN_MINUTES = 15L
    private val unb64 = Base64.getUrlDecoder()
    private val b64 = Base64.getUrlEncoder().withoutPadding()

    class Scanned(val id: String, val computerKey: PublicKey, val code: String) {
        val fileName: String = fileName(id)
        val confirmCode: String get() = confirmCode(code)
    }

    /** A request for the key, found in Drive. */
    class JoinRequest(val scanned: Scanned, val device: String, val at: Long)

    /** On a device without the key: a one-off key pair to receive it with. */
    class Offer {
        val id: String = b64.encodeToString(ByteArray(9).also { SecureRandom().nextBytes(it) })
        private val pair = newKeyPair()
        val code: String = PREFIX + id + ":" + b64.encodeToString(pair.public.encoded)
        val fileName: String = fileName(id)
        val joinFileName: String = "$JOIN_PREFIX$id.json"
        val confirmCode: String get() = confirmCode(code)

        fun joinRequest(device: String, now: Long = System.currentTimeMillis()): ByteArray =
            JSONObject().put("code", code).put("device", device).put("at", now).toString().toByteArray()

        /** Opens what the other device sent: the sync key. */
        fun open(blob: ByteArray): ByteArray {
            val buf = ByteBuffer.wrap(blob)
            val len = buf.short.toInt() and 0xffff
            require(blob.size > 2 + len + 12) { "Too short" }
            val theirs = ByteArray(len).also { buf.get(it) }
            val iv = ByteArray(12).also { buf.get(it) }
            val sealed = ByteArray(buf.remaining()).also { buf.get(it) }
            val pub = KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(theirs))
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(shared(pair.private, pub, id), "AES"), GCMParameterSpec(128, iv))
            return cipher.doFinal(sealed).also { require(it.size == 32) { "Wrong key size" } }
        }
    }

    /** Six digits both devices show, so you can check you're handing the key to the right one. */
    fun confirmCode(code: String): String {
        val h = MessageDigest.getInstance("SHA-256").digest(code.trim().toByteArray())
        val n = (ByteBuffer.wrap(h, 0, 4).int.toLong() and 0xffffffffL) % 1_000_000
        return "%03d %03d".format(n / 1000, n % 1000)
    }

    fun parseJoinRequest(bytes: ByteArray): JoinRequest? = runCatching {
        val o = JSONObject(String(bytes))
        val scanned = parse(o.getString("code")) ?: return null
        JoinRequest(scanned, o.optString("device", "A new device").ifEmpty { "A new device" }.take(60), o.optLong("at"))
    }.getOrNull()

    fun fileName(id: String) = "memi-pair-$id.bin"

    /** Reads a scanned QR code, or null if it isn't a Memi pairing code. */
    fun parse(code: String): Scanned? = runCatching {
        val trimmed = code.trim()
        if (!trimmed.startsWith(PREFIX)) return null
        val (id, key) = trimmed.removePrefix(PREFIX).split(':', limit = 2)
        Scanned(id, KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(unb64.decode(key))), trimmed)
    }.getOrNull()

    /** On a device with the key: encrypt [syncKey] so only the device that offered the code can read it. */
    fun sealFor(scanned: Scanned, syncKey: ByteArray): ByteArray {
        val mine = newKeyPair()
        val secret = shared(mine.private, scanned.computerKey, scanned.id)
        val iv = ByteArray(12).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(secret, "AES"), GCMParameterSpec(128, iv))
        val sealed = cipher.doFinal(syncKey)
        val pub = mine.public.encoded
        return ByteBuffer.allocate(2 + pub.size + iv.size + sealed.size)
            .putShort(pub.size.toShort()).put(pub).put(iv).put(sealed).array()
    }

    private fun newKeyPair(): KeyPair =
        KeyPairGenerator.getInstance("EC").apply { initialize(ECGenParameterSpec("secp256r1")) }.generateKeyPair()

    /** ECDH, then HKDF-SHA256 (RFC 5869) bound to the pairing id. */
    private fun shared(priv: PrivateKey, pub: PublicKey, id: String): ByteArray {
        val ikm = KeyAgreement.getInstance("ECDH").apply { init(priv); doPhase(pub, true) }.generateSecret()
        val prk = Mac.getInstance("HmacSHA256").apply { init(SecretKeySpec(ByteArray(32), "HmacSHA256")) }.doFinal(ikm)
        val info = "memi-pair-v1:$id".toByteArray() + byteArrayOf(1)
        return Mac.getInstance("HmacSHA256").apply { init(SecretKeySpec(prk, "HmacSHA256")) }.doFinal(info)
    }
}
