package app.memi.data

import app.memi.engine.Engine
import app.memi.engine.MemiException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import app.memi.engine.Collection as EngineCollection
import app.memi.engine.Note as EngineNote
import app.memi.engine.Person as EnginePerson

/**
 * The screens' way into memi-engine, the Rust engine every Memi app shares (ffi/ in the
 * repository). The engine does the edits (with the same rules on every device), saves them to
 * memi.db, and keeps the edit times and deletions sync needs. This class turns its records into
 * the screens' own and publishes them as flows.
 */
class Repository(private val engine: Engine) {

    private val _people = MutableStateFlow<List<Person>>(emptyList())
    private val _collections = MutableStateFlow<List<NoteCollection>>(emptyList())
    private val _notes = MutableStateFlow<List<Note>>(emptyList())
    private var seen: ULong? = null

    val people: Flow<List<Person>> = _people.asStateFlow()
    val collections: Flow<List<NoteCollection>> = _collections.asStateFlow()
    val notes: Flow<List<Note>> = _notes.asStateFlow()

    /** Called after every edit made here (not after changes from your other devices). */
    var onEdit: (() -> Unit)? = null

    init { refresh() }

    /** Reads everything again if anything changed, here or from another device. */
    @Synchronized
    fun refresh() {
        if (engine.version() == seen) return
        val all = engine.everything()
        seen = all.version
        _people.value = all.people.map { it.toPerson() }
        _collections.value = all.collections.map { it.toCollection() }
        _notes.value = all.notes.map { it.toNote() }
    }

    private suspend fun <T> edit(block: Engine.() -> T): T = withContext(Dispatchers.IO) {
        val result = engine.block()
        refresh()
        onEdit?.invoke()
        result
    }

    /** An edit that returns nothing. Something already gone (removed on another device a moment ago) is left as it is. */
    private suspend fun change(block: Engine.() -> Unit) = edit {
        try { block() } catch (_: MemiException) {}
    }

    // People

    /** Adds someone with the three starter categories; Gift ideas builds up to their birthday. */
    suspend fun addPerson(person: Person): Long = edit { addPerson(person.toEngine()) }

    suspend fun updatePerson(person: Person) = change { updatePerson(person.toEngine()) }

    /** Removes someone, everything about them, and their photo. */
    suspend fun deletePerson(person: Person) = change { deletePerson(person.id) }

    suspend fun setPinned(person: Person, pinned: Boolean) = change { setPinned(person.id, pinned) }

    /** Moves someone one place earlier (-1) or later (+1) among the pinned or unpinned group. */
    suspend fun movePerson(person: Person, delta: Int) = change { movePerson(person.id, delta) }

    /** Drag and drop: puts someone in [target]'s place (joining the pinned or unpinned group they're dropped into). */
    suspend fun placePerson(id: Long, target: Long) = change { placePerson(id, target) }

    fun deletePhotoFile(path: String?) { if (path != null) engine.deletePhotoFile(path) }

    // Notes

    suspend fun addNote(personId: Long, collectionId: Long, text: String, dueAt: Long? = null): Long =
        edit { addNote(personId, collectionId, text, dueAt) }

    /** Drag and drop: puts a note in [target]'s place within their category. */
    suspend fun placeNote(id: Long, target: Long) = change { placeNote(id, target) }

    suspend fun updateNote(note: Note) = change { updateNote(note.toEngine()) }

    suspend fun deleteNote(note: Note) = change { deleteNote(note.id) }

    suspend fun deleteNoteIds(ids: List<Long>) = change { ids.forEach { runCatching { deleteNote(it) } } }

    /** Undo: puts a deleted note back, the same note on every device. */
    suspend fun restoreNote(note: Note) = change { restoreNote(note.toEngine()) }

    suspend fun markDone(note: Note, outcome: Int) = change { markDone(note.id, outcome) }

    suspend fun reopen(note: Note) = change { reopen(note.id) }

    // Categories (each belongs to one person)

    suspend fun addCollection(personId: Long, name: String, occasion: String, occasionDay: Long?, yearly: Boolean): Long =
        edit { addCollection(personId, name.trim(), occasion, occasionDay, yearly) }

    suspend fun updateCollection(collection: NoteCollection) = change { updateCollection(collection.copy(name = collection.name.trim()).toEngine()) }

    /** Moves a category one place earlier (-1) or later (+1) among that person's categories. */
    suspend fun moveCollection(collection: NoteCollection, delta: Int) = change { moveCollection(collection.id, delta) }

    /** Drag and drop: puts a category in [target]'s place among that person's categories. */
    suspend fun placeCollection(id: Long, target: Long) = change { placeCollection(id, target) }

    /** Removes a category. Its notes move to another of the person's categories so nothing is lost. */
    suspend fun deleteCollection(collection: NoteCollection) = change { deleteCollection(collection.id) }
}

// --- Between the engine's records and the screens' ---------------------------------------------------

private fun EnginePerson.toPerson() = Person(
    id = id, uid = uid, updatedAt = updatedAt, name = name, relationship = relationship, photoPath = photoPath,
    birthdayMonth = birthdayMonth, birthdayDay = birthdayDay, birthYear = birthYear, toneIndex = toneIndex,
    pinned = pinned, sortOrder = sortOrder, createdAt = createdAt,
)

private fun Person.toEngine() = EnginePerson(
    id = id, uid = uid, updatedAt = updatedAt, name = name, relationship = relationship, photoPath = photoPath,
    birthdayMonth = birthdayMonth, birthdayDay = birthdayDay, birthYear = birthYear, toneIndex = toneIndex,
    pinned = pinned, sortOrder = sortOrder, createdAt = createdAt,
)

private fun EngineCollection.toCollection() = NoteCollection(
    id = id, uid = uid, updatedAt = updatedAt, personId = personId, name = name, kind = kind, slot = slot,
    sortOrder = sortOrder, occasion = occasion, occasionDay = occasionDay, yearly = yearly,
)

private fun NoteCollection.toEngine() = EngineCollection(
    id = id, uid = uid, updatedAt = updatedAt, personId = personId, name = name, kind = kind, slot = slot,
    sortOrder = sortOrder, occasion = occasion, occasionDay = occasionDay, yearly = yearly,
)

private fun EngineNote.toNote() = Note(
    id = id, uid = uid, updatedAt = updatedAt, personId = personId, collectionId = collectionId, text = text,
    mentionedAt = mentionedAt, firstMentionedAt = firstMentionedAt, mentionCount = mentionCount, dueAt = dueAt,
    status = status, outcome = outcome, doneAt = doneAt, createdAt = createdAt, sortOrder = sortOrder,
)

private fun Note.toEngine() = EngineNote(
    id = id, uid = uid, updatedAt = updatedAt, personId = personId, collectionId = collectionId, text = text,
    mentionedAt = mentionedAt, firstMentionedAt = firstMentionedAt, mentionCount = mentionCount, dueAt = dueAt,
    status = status, outcome = outcome, doneAt = doneAt, createdAt = createdAt, sortOrder = sortOrder,
)
