package app.memi.platform

import android.content.ContentValues
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.room.Room
import androidx.room.migration.Migration
import androidx.room.withTransaction
import androidx.sqlite.db.SupportSQLiteDatabase
import androidx.room.RoomDatabase
import app.memi.data.AppDatabase
import app.memi.data.Kind
import app.memi.data.LegacyUid
import app.memi.data.Occasion

/** Runs [block] as one transaction (used when restoring a backup). */
suspend fun AppDatabase.inTransaction(block: suspend () -> Unit) = withTransaction { block() }

/** Opens the database on Android, upgrading data saved by older versions of Memi. */
object AndroidDatabase {
    /** Adds pinning and your own order for people, keeping everything you've already saved. */
    private val MIGRATION_1_2 = object : Migration(1, 2) {
        override fun migrate(db: SupportSQLiteDatabase) {
            db.execSQL("ALTER TABLE people ADD COLUMN pinned INTEGER NOT NULL DEFAULT 0")
            db.execSQL("ALTER TABLE people ADD COLUMN sortOrder INTEGER NOT NULL DEFAULT 0")
        }
    }

    /**
     * Categories become per person and can be tied to a date. Each shared category is copied
     * to every person, their notes are moved across, and the shared originals are removed.
     * Gift ideas start out tied to the person's birthday.
     */
    private val MIGRATION_2_3 = object : Migration(2, 3) {
        override fun migrate(db: SupportSQLiteDatabase) {
            db.execSQL("ALTER TABLE collections ADD COLUMN personId INTEGER")
            db.execSQL("ALTER TABLE collections ADD COLUMN occasion TEXT NOT NULL DEFAULT '${Occasion.NONE}'")
            db.execSQL("ALTER TABLE collections ADD COLUMN occasionDay INTEGER")
            db.execSQL("ALTER TABLE collections ADD COLUMN yearly INTEGER NOT NULL DEFAULT 0")
            db.execSQL("CREATE INDEX IF NOT EXISTS index_collections_personId ON collections (personId)")
            db.execSQL("UPDATE collections SET occasion = '${Occasion.BIRTHDAY}' WHERE kind = '${Kind.GIFT}'")

            data class Shared(val id: Long, val name: String, val kind: String, val slot: Int, val sortOrder: Int, val occasion: String)
            val shared = mutableListOf<Shared>()
            db.query("SELECT id, name, kind, slot, sortOrder, occasion FROM collections WHERE personId IS NULL").use { c ->
                while (c.moveToNext()) shared += Shared(c.getLong(0), c.getString(1), c.getString(2), c.getInt(3), c.getInt(4), c.getString(5))
            }
            val people = mutableListOf<Long>()
            db.query("SELECT id FROM people").use { c -> while (c.moveToNext()) people += c.getLong(0) }

            for (p in people) {
                for (s in shared) {
                    val values = ContentValues().apply {
                        put("personId", p)
                        put("name", s.name)
                        put("kind", s.kind)
                        put("slot", s.slot)
                        put("sortOrder", s.sortOrder)
                        put("occasion", s.occasion)
                        put("yearly", 0)
                    }
                    val newId = db.insert("collections", SQLiteDatabase.CONFLICT_NONE, values)
                    db.execSQL("UPDATE notes SET collectionId = ? WHERE personId = ? AND collectionId = ?", arrayOf<Any>(newId, p, s.id))
                }
            }
            db.execSQL("DELETE FROM collections WHERE personId IS NULL")
        }
    }

    /**
     * Ready for merging (Memi 0.6): every record gets a permanent uid and an edit time, notes get
     * their own order, and deletions are remembered in `tombstones`. Existing records get the
     * same uids every other upgraded device gives them (see Uids.kt), so two devices that were
     * already syncing match up instead of doubling everything.
     */
    private val MIGRATION_3_4 = object : Migration(3, 4) {
        override fun migrate(db: SupportSQLiteDatabase) {
            for (t in listOf("people", "collections", "notes")) {
                db.execSQL("ALTER TABLE $t ADD COLUMN uid TEXT NOT NULL DEFAULT ''")
                db.execSQL("ALTER TABLE $t ADD COLUMN updatedAt INTEGER NOT NULL DEFAULT 0")
            }
            db.execSQL("ALTER TABLE notes ADD COLUMN sortOrder INTEGER NOT NULL DEFAULT 0")
            db.execSQL("UPDATE notes SET sortOrder = -mentionedAt")
            db.execSQL("CREATE TABLE IF NOT EXISTS tombstones (uid TEXT NOT NULL, deletedAt INTEGER NOT NULL, PRIMARY KEY(uid))")

            val people = mutableListOf<Pair<Long, Long>>()
            db.query("SELECT id, createdAt FROM people").use { c -> while (c.moveToNext()) people += c.getLong(0) to c.getLong(1) }
            for ((id, created) in people) db.execSQL("UPDATE people SET uid = ? WHERE id = ?", arrayOf<Any>(LegacyUid.person(id, created), id))

            val cols = mutableListOf<Triple<Long, Long?, Long?>>()
            db.query("SELECT c.id, c.personId, p.createdAt FROM collections c LEFT JOIN people p ON p.id = c.personId").use { c ->
                while (c.moveToNext()) cols += Triple(c.getLong(0), if (c.isNull(1)) null else c.getLong(1), if (c.isNull(2)) null else c.getLong(2))
            }
            for ((id, pid, pc) in cols) db.execSQL("UPDATE collections SET uid = ? WHERE id = ?", arrayOf<Any>(LegacyUid.collection(id, pid, pc), id))

            val notes = mutableListOf<Pair<Long, Long>>()
            db.query("SELECT id, createdAt FROM notes").use { c -> while (c.moveToNext()) notes += c.getLong(0) to c.getLong(1) }
            for ((id, created) in notes) db.execSQL("UPDATE notes SET uid = ? WHERE id = ?", arrayOf<Any>(LegacyUid.note(id, created), id))

            for (t in listOf("people", "collections", "notes")) db.execSQL("CREATE INDEX IF NOT EXISTS index_${t}_uid ON $t (uid)")
        }
    }

    /** The current time in milliseconds, in SQL. */
    private const val NOW = "CAST(ROUND((julianday('now') - 2440587.5) * 86400000) AS INTEGER)"

    /**
     * The triggers that keep uids, edit times and deletions up to date, the same as the desktop
     * app's (desktop/src/store.rs). Room doesn't manage triggers, so they're (re)created whenever
     * the database opens.
     */
    private fun syncTriggers(db: SupportSQLiteDatabase) {
        for (t in listOf("people", "collections", "notes")) {
            // A new row gets a uid and an edit time unless it came with them (from a merge).
            db.execSQL(
                """CREATE TRIGGER IF NOT EXISTS ${t}_stamp_insert AFTER INSERT ON $t
                WHEN NEW.uid = '' OR NEW.updatedAt = 0 BEGIN
                    UPDATE $t SET
                        uid = CASE WHEN NEW.uid = '' THEN lower(hex(randomblob(16))) ELSE NEW.uid END,
                        updatedAt = CASE WHEN NEW.updatedAt = 0 THEN $NOW ELSE NEW.updatedAt END
                    WHERE id = NEW.id;
                END"""
            )
            // Something brought back (Undo, or edited elsewhere after a deletion) is no longer deleted.
            db.execSQL(
                """CREATE TRIGGER IF NOT EXISTS ${t}_revive AFTER INSERT ON $t BEGIN
                    DELETE FROM tombstones WHERE uid = (SELECT uid FROM $t WHERE id = NEW.id);
                END"""
            )
            // A record saved from an out-of-date copy keeps its uid.
            db.execSQL(
                """CREATE TRIGGER IF NOT EXISTS ${t}_keep_uid AFTER UPDATE ON $t
                WHEN NEW.uid = '' AND OLD.uid != '' BEGIN
                    UPDATE $t SET uid = OLD.uid WHERE id = NEW.id;
                END"""
            )
            // Any change made here moves the edit time on, unless the change set it (a merge does).
            db.execSQL(
                """CREATE TRIGGER IF NOT EXISTS ${t}_stamp_update AFTER UPDATE ON $t
                WHEN NEW.updatedAt = OLD.updatedAt BEGIN
                    UPDATE $t SET updatedAt = $NOW WHERE id = NEW.id;
                END"""
            )
            db.execSQL(
                """CREATE TRIGGER IF NOT EXISTS ${t}_tombstone AFTER DELETE ON $t
                WHEN OLD.uid != '' BEGIN
                    INSERT OR REPLACE INTO tombstones (uid, deletedAt) VALUES (OLD.uid, $NOW);
                END"""
            )
        }
    }

    fun create(context: Context): AppDatabase =
        Room.databaseBuilder(context, AppDatabase::class.java, "memi.db")
            .addMigrations(MIGRATION_1_2, MIGRATION_2_3, MIGRATION_3_4)
            .addCallback(object : RoomDatabase.Callback() {
                override fun onOpen(db: SupportSQLiteDatabase) = syncTriggers(db)
            })
            .build()
}
