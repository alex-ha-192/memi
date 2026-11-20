package app.memi.data

import androidx.room.Database
import androidx.room.RoomDatabase

/**
 * The same database on phone and desktop. Each app opens it its own way: Android in
 * platform/AndroidDatabase.kt (with migrations from older versions), desktop with bundled SQLite.
 */
@Database(
    entities = [Person::class, NoteCollection::class, Note::class],
    version = 3,
    exportSchema = true,
)
abstract class AppDatabase : RoomDatabase() {
    abstract fun people(): PersonDao
    abstract fun collections(): CollectionDao
    abstract fun notes(): NoteDao
}
