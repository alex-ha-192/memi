package app.memi.platform

import android.content.ContentValues
import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.room.Room
import androidx.room.migration.Migration
import androidx.room.withTransaction
import androidx.sqlite.db.SupportSQLiteDatabase
import app.memi.data.AppDatabase
import app.memi.data.Kind
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

    fun create(context: Context): AppDatabase =
        Room.databaseBuilder(context, AppDatabase::class.java, "memi.db")
            .addMigrations(MIGRATION_1_2, MIGRATION_2_3)
            .build()
}
