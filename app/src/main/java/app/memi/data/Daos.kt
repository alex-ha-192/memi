package app.memi.data

import androidx.room.Dao
import androidx.room.Delete
import androidx.room.Insert
import androidx.room.Query
import androidx.room.Update
import kotlinx.coroutines.flow.Flow

@Dao
interface PersonDao {
    @Query("SELECT * FROM people ORDER BY pinned DESC, sortOrder, name COLLATE NOCASE")
    fun observeAll(): Flow<List<Person>>

    @Query("SELECT * FROM people ORDER BY pinned DESC, sortOrder, name COLLATE NOCASE")
    suspend fun all(): List<Person>

    @Query("SELECT * FROM people WHERE id = :id")
    suspend fun get(id: Long): Person?

    @Insert
    suspend fun insert(person: Person): Long

    @Insert
    suspend fun insertAll(people: List<Person>)

    @Update
    suspend fun update(person: Person)

    @Update
    suspend fun updateAll(people: List<Person>)

    @Delete
    suspend fun delete(person: Person)

    @Query("DELETE FROM people")
    suspend fun clear()
}

@Dao
interface CollectionDao {
    @Query("SELECT * FROM collections ORDER BY personId, sortOrder")
    fun observeAll(): Flow<List<NoteCollection>>

    @Query("SELECT * FROM collections ORDER BY personId, sortOrder")
    suspend fun all(): List<NoteCollection>

    @Query("SELECT * FROM collections WHERE personId = :personId ORDER BY sortOrder")
    suspend fun forPerson(personId: Long): List<NoteCollection>

    @Insert
    suspend fun insert(collection: NoteCollection): Long

    @Insert
    suspend fun insertAll(collections: List<NoteCollection>)

    @Update
    suspend fun update(collection: NoteCollection)

    @Update
    suspend fun updateAll(collections: List<NoteCollection>)

    @Delete
    suspend fun delete(collection: NoteCollection)

    @Query("DELETE FROM collections WHERE personId = :personId")
    suspend fun deleteForPerson(personId: Long)

    @Query("DELETE FROM collections")
    suspend fun clear()
}

@Dao
interface NoteDao {
    @Query("SELECT * FROM notes ORDER BY sortOrder, mentionedAt DESC")
    fun observeAll(): Flow<List<Note>>

    @Query("SELECT * FROM notes")
    suspend fun all(): List<Note>

    @Insert
    suspend fun insert(note: Note): Long

    @Insert
    suspend fun insertAll(notes: List<Note>): List<Long>

    @Update
    suspend fun update(note: Note)

    @Delete
    suspend fun delete(note: Note)

    @Query("DELETE FROM notes WHERE id IN (:ids)")
    suspend fun deleteIds(ids: List<Long>)

    @Query("UPDATE notes SET collectionId = :to WHERE collectionId = :from")
    suspend fun moveAll(from: Long, to: Long)

    @Query("DELETE FROM notes")
    suspend fun clear()
}

@Dao
interface TombstoneDao {
    @Query("SELECT * FROM tombstones")
    suspend fun all(): List<Tombstone>
}
