package app.memi.sync

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.security.SecureRandom
import java.util.zip.GZIPInputStream
import java.util.zip.GZIPOutputStream
import javax.crypto.AEADBadTagException
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * End-to-end encryption for sync. A random AES-256 key is made on the device the first time sync
 * is turned on, and every backup is sealed with AES-GCM before it leaves. Google Drive only ever
 * stores ciphertext. How the key is kept and moved to other devices is up to each platform's
 * [KeyVault]; the recovery key always works as a fallback.
 *
 * File layout: "MEMI1" | 16 reserved bytes | 12-byte IV | AES-GCM(gzip(json)) with a 128-bit tag.
 */
object Crypto {
    private val MAGIC = "MEMI1".toByteArray(Charsets.US_ASCII)
    private const val SALT = 16
    private const val IV = 12

    class WrongKey : Exception("This key doesn't open the backup.")

    fun newKey(): ByteArray = ByteArray(32).also { SecureRandom().nextBytes(it) }

    /** The key as text, for the optional recovery key shown in settings. */
    fun keyToText(key: ByteArray): String = key.joinToString("") { "%02x".format(it) }.chunked(8).joinToString("-")

    fun keyFromText(text: String): ByteArray? {
        val hex = text.filter { it.isLetterOrDigit() }.lowercase()
        if (hex.length != 64 || hex.any { it !in "0123456789abcdef" }) return null
        return ByteArray(32) { hex.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }

    fun isBackup(blob: ByteArray): Boolean =
        blob.size > MAGIC.size + SALT + IV && blob.copyOfRange(0, MAGIC.size).contentEquals(MAGIC)

    fun seal(key: ByteArray, plain: ByteArray): ByteArray {
        val salt = ByteArray(SALT)
        val iv = ByteArray(IV).also { SecureRandom().nextBytes(it) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "AES"), GCMParameterSpec(128, iv))
        cipher.updateAAD(MAGIC)
        return MAGIC + salt + iv + cipher.doFinal(gzip(plain))
    }

    fun open(key: ByteArray, blob: ByteArray): ByteArray {
        require(isBackup(blob)) { "This isn't a Your Memi backup." }
        val ivStart = MAGIC.size + SALT
        val iv = blob.copyOfRange(ivStart, ivStart + IV)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, SecretKeySpec(key, "AES"), GCMParameterSpec(128, iv))
        cipher.updateAAD(MAGIC)
        val plain = try {
            cipher.doFinal(blob, ivStart + IV, blob.size - ivStart - IV)
        } catch (e: AEADBadTagException) {
            throw WrongKey()
        }
        return gunzip(plain)
    }

    private fun gzip(b: ByteArray): ByteArray = ByteArrayOutputStream().also { out -> GZIPOutputStream(out).use { it.write(b) } }.toByteArray()
    private fun gunzip(b: ByteArray): ByteArray = GZIPInputStream(ByteArrayInputStream(b)).use { it.readBytes() }
}
