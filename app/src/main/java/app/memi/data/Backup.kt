package app.memi.data

import android.content.ContentValues
import android.database.sqlite.SQLiteDatabase
import app.memi.platform.PlatformPhotos
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.Base64
import java.util.UUID

/**
 * Everything in Memi (people, their categories, notes, photos and recent deletions) as one JSON
 * document, in exactly the same shape as the desktop and web apps' (core/src/backup.rs). It's only
 * ever stored encrypted; see sync/Crypto.kt.
 *
 * Format 2 (Memi 0.6) adds a permanent `uid` and an `updatedAt` time to every record, refers to
 * parents by uid, and lists deletions as `tombstones`, so copies can be merged (Merge.kt). Format 1
 * backups (Memi 0.5) still open, with the same uids an upgraded device gives their records.
 */
object Backup {
    const val FORMAT = 2

    class Doc(
        val snapshot: Snapshot,
        /** Photo bytes by person uid. */
        val photos: Map<String, ByteArray>,
        /** Written by Memi 0.5 (no edit times or deletions recorded). */
        val legacy: Boolean,
    )

    class TooNew : Exception("This backup was made by a newer version of Memi. Update this app to open it.")

    // --- Reading this phone's copy and writing a merged one back --------------------------------

    suspend fun snapshot(db: AppDatabase): Snapshot {
        val people = db.people().all()
        val cols = db.collections().all()
        val personUid = people.associate { it.id to it.uid }
        val colUid = cols.associate { it.id to it.uid }
        return Snapshot(
            people = people,
            collections = cols.map { CollectionRec(it, it.personId?.let(personUid::get)) },
            notes = db.notes().all().mapNotNull { n ->
                val p = personUid[n.personId] ?: return@mapNotNull null
                val c = colUid[n.collectionId] ?: return@mapNotNull null
                NoteRec(n, p, c)
            },
            tombstones = db.tombstones().all(),
        )
    }

    /**
     * Makes this phone's copy match [s], touching only what differs. People in [fromRemote] take
     * their photo from [photos]. Keeps the edit times exactly as they are in [s].
     */
    suspend fun apply(db: AppDatabase, s: Snapshot, fromRemote: Set<String>, photos: Map<String, ByteArray>) = withContext(Dispatchers.IO) {
        val before = snapshot(db)
        val oldPeople = before.people.associateBy { it.uid }
        val oldCols = before.collections.associateBy { it.c.uid }
        val oldNotes = before.notes.associateBy { it.n.uid }
        val unusedPhotos = mutableListOf<String>()
        val newPhotos = mutableListOf<String>()

        try {
            db.runInTransaction(Runnable {
                val w = db.openHelper.writableDatabase
                fun upsert(table: String, id: Long?, v: ContentValues): Long =
                    if (id != null) { w.update(table, SQLiteDatabase.CONFLICT_NONE, v, "id = ?", arrayOf(id)); id }
                    else w.insert(table, SQLiteDatabase.CONFLICT_NONE, v)

                val keep = s.people.map { it.uid }.toSet() + s.collections.map { it.c.uid } + s.notes.map { it.n.uid }
                before.notes.filter { it.n.uid !in keep }.forEach { w.delete("notes", "id = ?", arrayOf(it.n.id)) }
                before.collections.filter { it.c.uid !in keep }.forEach { w.delete("collections", "id = ?", arrayOf(it.c.id)) }
                before.people.filter { it.uid !in keep }.forEach {
                    w.delete("people", "id = ?", arrayOf(it.id))
                    it.photoPath?.let(unusedPhotos::add)
                }

                val personId = before.people.associate { it.uid to it.id }.toMutableMap()
                for (p in s.people) {
                    val old = oldPeople[p.uid]
                    if (old != null && old.updatedAt == p.updatedAt) continue
                    val path = if (p.uid in fromRemote) photos[p.uid]?.let { savePhoto(it) }?.also(newPhotos::add) else old?.photoPath
                    if (old?.photoPath != null && old.photoPath != path) unusedPhotos += old.photoPath
                    val v = ContentValues().apply {
                        put("uid", p.uid); put("name", p.name); put("relationship", p.relationship); put("photoPath", path)
                        put("birthdayMonth", p.birthdayMonth); put("birthdayDay", p.birthdayDay); put("birthYear", p.birthYear)
                        put("toneIndex", p.toneIndex); put("pinned", p.pinned); put("sortOrder", p.sortOrder)
                        put("createdAt", p.createdAt); put("updatedAt", p.updatedAt)
                    }
                    personId[p.uid] = upsert("people", old?.id, v)
                }

                val colId = before.collections.associate { it.c.uid to it.c.id }.toMutableMap()
                for (rec in s.collections) {
                    val c = rec.c
                    val pid = rec.personUid?.let(personId::get) ?: continue
                    val old = oldCols[c.uid]
                    if (old != null && old.c.updatedAt == c.updatedAt && old.c.personId == pid) continue
                    val v = ContentValues().apply {
                        put("uid", c.uid); put("personId", pid); put("name", c.name); put("kind", c.kind); put("slot", c.slot)
                        put("sortOrder", c.sortOrder); put("occasion", c.occasion); put("occasionDay", c.occasionDay)
                        put("yearly", c.yearly); put("updatedAt", c.updatedAt)
                    }
                    colId[c.uid] = upsert("collections", old?.c?.id, v)
                }

                for (rec in s.notes) {
                    val n = rec.n
                    val pid = personId[rec.personUid] ?: continue
                    val cid = colId[rec.collectionUid] ?: continue
                    val old = oldNotes[n.uid]
                    if (old != null && old.n.updatedAt == n.updatedAt && old.n.collectionId == cid) continue
                    val v = ContentValues().apply {
                        put("uid", n.uid); put("personId", pid); put("collectionId", cid); put("text", n.text)
                        put("mentionedAt", n.mentionedAt); put("firstMentionedAt", n.firstMentionedAt); put("mentionCount", n.mentionCount)
                        put("dueAt", n.dueAt); put("status", n.status); put("outcome", n.outcome); put("doneAt", n.doneAt)
                        put("createdAt", n.createdAt); put("sortOrder", n.sortOrder); put("updatedAt", n.updatedAt)
                    }
                    upsert("notes", old?.n?.id, v)
                    // A note that only moved category keeps its time; undo the trigger's stamp.
                    if (old != null) {
                        w.execSQL("UPDATE notes SET updatedAt = ? WHERE id = ? AND updatedAt != ?", arrayOf<Any>(n.updatedAt, old.n.id, n.updatedAt))
                    }
                }

                w.execSQL("DELETE FROM tombstones")
                for (t in s.tombstones) {
                    w.insert("tombstones", SQLiteDatabase.CONFLICT_REPLACE, ContentValues().apply { put("uid", t.uid); put("deletedAt", t.deletedAt) })
                }
            })
        } catch (e: Exception) {
            newPhotos.forEach { PlatformPhotos.delete(it) }
            throw e
        }
        unusedPhotos.forEach { PlatformPhotos.delete(it) }
    }

    private fun savePhoto(bytes: ByteArray): String {
        val dir = PlatformPhotos.dir.apply { mkdirs() }
        return File(dir, "${UUID.randomUUID()}.jpg").apply { writeBytes(bytes) }.absolutePath
    }

    // --- The JSON document -----------------------------------------------------------------------

    fun export(s: Snapshot): ByteArray {
        val root = JSONObject()
            .put("format", FORMAT)
            .put("exportedAt", System.currentTimeMillis())
            .put("people", JSONArray().apply { s.people.forEach { put(it.toJson()) } })
            .put("collections", JSONArray().apply { s.collections.forEach { put(it.toJson()) } })
            .put("notes", JSONArray().apply { s.notes.forEach { put(it.toJson()) } })
            .put("tombstones", JSONArray().apply { s.tombstones.forEach { put(JSONObject().put("uid", it.uid).put("deletedAt", it.deletedAt)) } })
        return root.toString().toByteArray(Charsets.UTF_8)
    }

    fun import(bytes: ByteArray): Doc {
        val root = JSONObject(String(bytes, Charsets.UTF_8))
        if (root.optInt("format") > FORMAT) throw TooNew()
        val photos = HashMap<String, ByteArray>()

        val personUid = HashMap<Long, String>()
        val personCreated = HashMap<Long, Long>()
        val people = root.optJSONArray("people").objects().map { o ->
            val id = o.getLong("id")
            val created = o.optLong("createdAt", System.currentTimeMillis())
            val p = Person(
                id = id, uid = o.uid { LegacyUid.person(id, created) }, updatedAt = o.optLong("updatedAt", 0),
                name = o.getString("name"), relationship = o.optString("relationship"),
                birthdayMonth = o.optIntOrNull("birthdayMonth")?.takeIf { it in 1..12 },
                birthdayDay = o.optIntOrNull("birthdayDay")?.takeIf { it in 1..31 },
                birthYear = o.optIntOrNull("birthYear"),
                toneIndex = o.optInt("toneIndex"), pinned = o.optBoolean("pinned"), sortOrder = o.optInt("sortOrder"),
                createdAt = created,
            )
            o.optStringOrNull("photo")?.takeIf { it.isNotEmpty() }?.let { runCatching { photos[p.uid] = Base64.getDecoder().decode(it) } }
            personUid[id] = p.uid
            personCreated[id] = created
            p
        }

        val colUid = HashMap<Long, String>()
        val cols = root.optJSONArray("collections").objects().map { o ->
            val id = o.getLong("id")
            val pid = o.optLongOrNull("personId")
            val c = NoteCollection(
                id = id, uid = o.uid { LegacyUid.collection(id, pid, pid?.let(personCreated::get)) }, updatedAt = o.optLong("updatedAt", 0),
                personId = pid, name = o.getString("name"), kind = o.optString("kind", Kind.CUSTOM),
                slot = Math.floorMod(o.optInt("slot"), Slot.COUNT), sortOrder = o.optInt("sortOrder"),
                occasion = o.optString("occasion", Occasion.NONE), occasionDay = o.optLongOrNull("occasionDay"),
                yearly = o.optBoolean("yearly"),
            )
            colUid[id] = c.uid
            CollectionRec(c, o.optStringOrNull("personUid") ?: pid?.let(personUid::get))
        }

        val notes = root.optJSONArray("notes").objects().mapNotNull { o ->
            val id = o.getLong("id")
            val pid = o.getLong("personId")
            val cid = o.getLong("collectionId")
            val pu = o.optStringOrNull("personUid") ?: personUid[pid] ?: return@mapNotNull null
            val cu = o.optStringOrNull("collectionUid") ?: colUid[cid] ?: return@mapNotNull null
            val created = o.optLong("createdAt", System.currentTimeMillis())
            val mentioned = o.getLong("mentionedAt")
            NoteRec(
                Note(
                    id = id, uid = o.uid { LegacyUid.note(id, created) }, updatedAt = o.optLong("updatedAt", 0),
                    personId = pid, collectionId = cid, text = o.getString("text"),
                    mentionedAt = mentioned, firstMentionedAt = o.optLong("firstMentionedAt", mentioned),
                    mentionCount = o.optInt("mentionCount", 1), dueAt = o.optLongOrNull("dueAt"),
                    status = o.optInt("status"), outcome = o.optInt("outcome"), doneAt = o.optLongOrNull("doneAt"),
                    createdAt = created, sortOrder = o.optLongOrNull("sortOrder") ?: -mentioned,
                ),
                pu, cu,
            )
        }

        val tombstones = root.optJSONArray("tombstones").objects().mapNotNull { o ->
            val uid = o.optStringOrNull("uid") ?: return@mapNotNull null
            val at = o.optLongOrNull("deletedAt") ?: return@mapNotNull null
            Tombstone(uid, at)
        }
        return Doc(Snapshot(people, cols, notes, tombstones), photos, legacy = root.optInt("format") < 2)
    }

    /** The photo to send for [p], if they have one. */
    fun photoBytes(p: Person): ByteArray? = p.photoPath?.let { File(it) }?.takeIf { it.exists() }?.readBytes()

    private fun JSONArray?.objects(): List<JSONObject> = if (this == null) emptyList() else (0 until length()).map { getJSONObject(it) }
    private fun JSONObject.optLongOrNull(key: String): Long? = if (has(key) && !isNull(key)) getLong(key) else null
    private fun JSONObject.optIntOrNull(key: String): Int? = if (has(key) && !isNull(key)) getInt(key) else null
    private fun JSONObject.optStringOrNull(key: String): String? = if (has(key) && !isNull(key)) getString(key) else null
    private fun JSONObject.uid(legacy: () -> String) = optStringOrNull("uid")?.takeIf { it.isNotEmpty() } ?: legacy()
    private fun Any?.orNull(): Any = this ?: JSONObject.NULL

    private fun Person.toJson() = JSONObject()
        .put("id", id).put("uid", uid).put("updatedAt", updatedAt)
        .put("name", name).put("relationship", relationship)
        .put("photo", photoBytes(this)?.let { Base64.getEncoder().encodeToString(it) }.orNull())
        .put("birthdayMonth", birthdayMonth.orNull()).put("birthdayDay", birthdayDay.orNull()).put("birthYear", birthYear.orNull())
        .put("toneIndex", toneIndex).put("pinned", pinned).put("sortOrder", sortOrder).put("createdAt", createdAt)

    private fun CollectionRec.toJson() = JSONObject()
        .put("id", c.id).put("uid", c.uid).put("updatedAt", c.updatedAt)
        .put("personId", c.personId.orNull()).put("personUid", personUid.orNull())
        .put("name", c.name).put("kind", c.kind).put("slot", c.slot).put("sortOrder", c.sortOrder)
        .put("occasion", c.occasion).put("occasionDay", c.occasionDay.orNull()).put("yearly", c.yearly)

    private fun NoteRec.toJson() = JSONObject()
        .put("id", n.id).put("uid", n.uid).put("updatedAt", n.updatedAt)
        .put("personId", n.personId).put("personUid", personUid).put("collectionId", n.collectionId).put("collectionUid", collectionUid)
        .put("text", n.text).put("mentionedAt", n.mentionedAt).put("firstMentionedAt", n.firstMentionedAt).put("mentionCount", n.mentionCount)
        .put("dueAt", n.dueAt.orNull()).put("status", n.status).put("outcome", n.outcome).put("doneAt", n.doneAt.orNull())
        .put("createdAt", n.createdAt).put("sortOrder", n.sortOrder)
}
