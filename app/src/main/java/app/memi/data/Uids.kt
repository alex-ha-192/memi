package app.memi.data

import java.security.MessageDigest
import java.security.SecureRandom

/** A new random uid: 32 hex characters, like the database's lower(hex(randomblob(16))). */
fun newUid(): String = ByteArray(16).also { SecureRandom().nextBytes(it) }.toHex()

private fun ByteArray.toHex() = joinToString("") { "%02x".format(it) }

/**
 * The uids records saved before Memi 0.6 get, worked out from things that were already the same
 * on every synced device (row ids and creation times). Byte-for-byte the same as
 * core/src/model.rs `legacy_uid`, so a phone and a computer that upgrade separately agree.
 */
object LegacyUid {
    private fun hash(s: String) = MessageDigest.getInstance("SHA-256").digest(s.toByteArray()).copyOf(16).toHex()
    fun person(id: Long, createdAt: Long) = hash("memi-person:$id:$createdAt")
    fun collection(id: Long, personId: Long?, personCreatedAt: Long?) = hash("memi-collection:$id:${personId ?: 0}:${personCreatedAt ?: 0}")
    fun note(id: Long, createdAt: Long) = hash("memi-note:$id:$createdAt")
}
