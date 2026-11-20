package app.memi.data

import app.memi.platform.PlatformPhotos
import app.memi.platform.inTransaction
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.Base64
import java.util.UUID

/**
 * Turns everything in Memi (people, their categories, notes and photos) into one JSON document
 * and back. The JSON is only ever stored encrypted; see sync/Crypto.kt.
 */
object Backup {
    private const val FORMAT = 1

    suspend fun export(db: AppDatabase): ByteArray {
        val people = db.people().all()
        val collections = db.collections().all()
        val notes = db.notes().all()
        val root = JSONObject()
            .put("format", FORMAT)
            .put("exportedAt", System.currentTimeMillis())
            .put("people", JSONArray().apply { people.forEach { put(it.toJson()) } })
            .put("collections", JSONArray().apply { collections.forEach { put(it.toJson()) } })
            .put("notes", JSONArray().apply { notes.forEach { put(it.toJson()) } })
        return root.toString().toByteArray(Charsets.UTF_8)
    }

    /** Replaces everything on this phone with the backup's contents, all at once. */
    suspend fun import(db: AppDatabase, bytes: ByteArray) {
        val root = JSONObject(String(bytes, Charsets.UTF_8))
        require(root.optInt("format") <= FORMAT) { "This backup was made by a newer version of Your Memi." }
        val oldPhotos = db.people().all().mapNotNull { it.photoPath }

        val people = root.getJSONArray("people").objects().map { personFrom(it) }
        val collections = root.getJSONArray("collections").objects().map(::collectionFrom)
        val notes = root.getJSONArray("notes").objects().map(::noteFrom)

        db.inTransaction {
            db.notes().clear()
            db.collections().clear()
            db.people().clear()
            db.people().insertAll(people)
            db.collections().insertAll(collections)
            db.notes().insertAll(notes)
        }
        val kept = people.mapNotNull { it.photoPath }.toSet()
        oldPhotos.filter { it !in kept }.forEach { PlatformPhotos.delete(it) }
    }

    private fun JSONArray.objects(): List<JSONObject> = (0 until length()).map { getJSONObject(it) }

    private fun JSONObject.optLongOrNull(key: String): Long? = if (has(key) && !isNull(key)) getLong(key) else null
    private fun JSONObject.optIntOrNull(key: String): Int? = if (has(key) && !isNull(key)) getInt(key) else null

    private fun Person.toJson() = JSONObject()
        .put("id", id).put("name", name).put("relationship", relationship)
        .put("photo", photoPath?.let { p -> File(p).takeIf { it.exists() }?.readBytes()?.let { Base64.getEncoder().encodeToString(it) } } ?: JSONObject.NULL)
        .put("birthdayMonth", birthdayMonth ?: JSONObject.NULL)
        .put("birthdayDay", birthdayDay ?: JSONObject.NULL)
        .put("birthYear", birthYear ?: JSONObject.NULL)
        .put("toneIndex", toneIndex).put("pinned", pinned).put("sortOrder", sortOrder).put("createdAt", createdAt)

    private fun personFrom(o: JSONObject): Person {
        val photo = o.optString("photo").takeIf { o.has("photo") && !o.isNull("photo") && it.isNotEmpty() }?.let { b64 ->
            val dir = PlatformPhotos.dir.apply { mkdirs() }
            File(dir, "${UUID.randomUUID()}.jpg").apply { writeBytes(Base64.getDecoder().decode(b64)) }.absolutePath
        }
        return Person(
            id = o.getLong("id"), name = o.getString("name"), relationship = o.optString("relationship"),
            photoPath = photo,
            birthdayMonth = o.optIntOrNull("birthdayMonth"), birthdayDay = o.optIntOrNull("birthdayDay"),
            birthYear = o.optIntOrNull("birthYear"),
            toneIndex = o.optInt("toneIndex"), pinned = o.optBoolean("pinned"), sortOrder = o.optInt("sortOrder"),
            createdAt = o.optLong("createdAt", System.currentTimeMillis()),
        )
    }

    private fun NoteCollection.toJson() = JSONObject()
        .put("id", id).put("personId", personId ?: JSONObject.NULL).put("name", name).put("kind", kind)
        .put("slot", slot).put("sortOrder", sortOrder).put("occasion", occasion)
        .put("occasionDay", occasionDay ?: JSONObject.NULL).put("yearly", yearly)

    private fun collectionFrom(o: JSONObject) = NoteCollection(
        id = o.getLong("id"), personId = o.optLongOrNull("personId"), name = o.getString("name"),
        kind = o.optString("kind", Kind.CUSTOM), slot = o.optInt("slot"), sortOrder = o.optInt("sortOrder"),
        occasion = o.optString("occasion", Occasion.NONE), occasionDay = o.optLongOrNull("occasionDay"),
        yearly = o.optBoolean("yearly"),
    )

    private fun Note.toJson() = JSONObject()
        .put("id", id).put("personId", personId).put("collectionId", collectionId).put("text", text)
        .put("mentionedAt", mentionedAt).put("firstMentionedAt", firstMentionedAt).put("mentionCount", mentionCount)
        .put("dueAt", dueAt ?: JSONObject.NULL).put("status", status).put("outcome", outcome)
        .put("doneAt", doneAt ?: JSONObject.NULL).put("createdAt", createdAt)

    private fun noteFrom(o: JSONObject) = Note(
        id = o.getLong("id"), personId = o.getLong("personId"), collectionId = o.getLong("collectionId"),
        text = o.getString("text"), mentionedAt = o.getLong("mentionedAt"),
        firstMentionedAt = o.optLong("firstMentionedAt", o.getLong("mentionedAt")),
        mentionCount = o.optInt("mentionCount", 1), dueAt = o.optLongOrNull("dueAt"),
        status = o.optInt("status"), outcome = o.optInt("outcome"), doneAt = o.optLongOrNull("doneAt"),
        createdAt = o.optLong("createdAt", System.currentTimeMillis()),
    )
}
