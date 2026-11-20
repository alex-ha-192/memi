package app.memi.sync

import java.nio.ByteBuffer
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
 */
object Pairing {
    private const val PREFIX = "memi:pair:1:"
    private val b64 = Base64.getUrlEncoder().withoutPadding()
    private val unb64 = Base64.getUrlDecoder()

    class Offer(val id: String, val keys: KeyPair) {
        /** What goes in the QR code. */
        val code: String = PREFIX + id + ":" + b64.encodeToString(keys.public.encoded)
        val fileName: String = fileName(id)
    }

    class Scanned(val id: String, val computerKey: PublicKey) {
        val fileName: String = fileName(id)
    }

    fun fileName(id: String) = "memi-pair-$id.bin"

    fun newOffer(): Offer = Offer(b64.encodeToString(ByteArray(9).also { SecureRandom().nextBytes(it) }), newKeyPair())

    /** Reads a scanned QR code, or null if it isn't a Memi pairing code. */
    fun parse(code: String): Scanned? = runCatching {
        if (!code.startsWith(PREFIX)) return null
        val (id, key) = code.removePrefix(PREFIX).split(':', limit = 2)
        Scanned(id, KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(unb64.decode(key))))
    }.getOrNull()

    /** On the phone: encrypt [syncKey] so only the computer that showed the code can read it. */
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

    /** On the computer: open what the phone sent. */
    fun open(offer: Offer, blob: ByteArray): ByteArray {
        val buf = ByteBuffer.wrap(blob)
        val pub = ByteArray(buf.short.toInt()).also { buf.get(it) }
        val iv = ByteArray(12).also { buf.get(it) }
        val sealed = ByteArray(buf.remaining()).also { buf.get(it) }
        val phoneKey = KeyFactory.getInstance("EC").generatePublic(X509EncodedKeySpec(pub))
        val secret = shared(offer.keys.private, phoneKey, offer.id)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(secret, "AES"), GCMParameterSpec(128, iv))
        return cipher.doFinal(sealed)
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
