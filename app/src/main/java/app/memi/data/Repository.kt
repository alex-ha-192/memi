package app.memi.data

import app.memi.platform.PlatformPhotos
import kotlinx.coroutines.flow.Flow

class Repository(private val db: AppDatabase) {

    /** Every edit moves the record's edit time on, so it wins over older copies when merging. */
    private fun now() = System.currentTimeMillis()

    val people: Flow<List<Person>> = db.people().observeAll()
    val collections: Flow<List<NoteCollection>> = db.collections().observeAll()
    val notes: Flow<List<Note>> = db.notes().observeAll()

    // People

    /** Adds someone with the three starter categories; Gift ideas builds up to their birthday. */
    suspend fun addPerson(person: Person): Long {
        val all = db.people().all()
        val id = db.people().insert(
            person.copy(toneIndex = all.size % 8, sortOrder = (all.maxOfOrNull { it.sortOrder } ?: 0) + 1)
        )
        db.collections().insertAll(
            listOf(
                NoteCollection(personId = id, name = "Gift ideas", kind = Kind.GIFT, slot = Slot.GIFT, sortOrder = 0, occasion = Occasion.BIRTHDAY),
                NoteCollection(personId = id, name = "Plans", kind = Kind.PLAN, slot = Slot.PLAN, sortOrder = 1),
                NoteCollection(personId = id, name = "Notes", kind = Kind.NOTE, slot = Slot.NOTE, sortOrder = 2),
            )
        )
        return id
    }

    suspend fun updatePerson(person: Person) = db.people().update(person.copy(updatedAt = now()))

    suspend fun deletePerson(person: Person) {
        PlatformPhotos.delete(person.photoPath)
        db.collections().deleteForPerson(person.id)
        db.people().delete(person)
    }

    suspend fun setPinned(person: Person, pinned: Boolean) {
        val all = db.people().all()
        val endOfGroup = (all.filter { it.pinned == pinned }.maxOfOrNull { it.sortOrder } ?: 0) + 1
        db.people().update(person.copy(pinned = pinned, sortOrder = endOfGroup, updatedAt = now()))
    }

    /** Moves someone one place earlier (-1) or later (+1) among the pinned or unpinned group. */
    suspend fun movePerson(person: Person, delta: Int) {
        val all = db.people().all()
        val group = all.filter { it.pinned == person.pinned }.toMutableList()
        val i = group.indexOfFirst { it.id == person.id }
        val j = i + delta
        if (i < 0 || j !in group.indices) return
        group.add(j, group.removeAt(i))
        db.people().updateAll(group.mapIndexed { index, p -> p.copy(sortOrder = index, updatedAt = now()) })
    }


    /**
     * Drag and drop: puts someone in [target]'s place (joining the pinned or unpinned group they're
     * dropped into). Same rules as the desktop's Store::place_person.
     */
    suspend fun placePerson(id: Long, target: Long) {
        val all = db.people().all()
        val t = all.firstOrNull { it.id == target } ?: return
        val me = all.firstOrNull { it.id == id } ?: return
        val inGroup = all.filter { it.pinned == t.pinned }
        val group = inGroup.filter { it.id != id }.toMutableList()
        var at = group.indexOfFirst { it.id == target }.let { if (it < 0) group.size else it }
        val old = inGroup.indexOfFirst { it.id == id }
        // Dropping onto someone further down puts you after them; further up, before them.
        if (old in 0..at) at += 1
        group.add(at.coerceAtMost(group.size), me)
        val now = now()
        db.people().updateAll(group.mapIndexedNotNull { i, p ->
            if (p.sortOrder == i && p.pinned == t.pinned) null else p.copy(sortOrder = i, pinned = t.pinned, updatedAt = now)
        })
    }

    fun deletePhotoFile(path: String?) = PlatformPhotos.delete(path)

    // Notes

    suspend fun addNote(personId: Long, collectionId: Long, text: String, dueAt: Long? = null): Long {
        val now = System.currentTimeMillis()
        return db.notes().insert(
            Note(personId = personId, collectionId = collectionId, text = text.trim(), mentionedAt = now, firstMentionedAt = now, dueAt = dueAt, sortOrder = -now)
        )
    }

    /** Drag and drop: puts a note in [target]'s place within their category. */
    suspend fun placeNote(id: Long, target: Long) {
        val all = db.notes().all()
        val me = all.firstOrNull { it.id == id } ?: return
        val t = all.firstOrNull { it.id == target } ?: return
        if (me.collectionId != t.collectionId) return
        val list = all.filter { it.collectionId == t.collectionId && it.status == t.status }
            .sortedWith(compareBy({ it.sortOrder }, { -it.mentionedAt }))
        val now = now()
        db.notes().updateAll(place(list, { it.id }, id, target).mapIndexedNotNull { i, n ->
            if (n.sortOrder == i.toLong()) null else n.copy(sortOrder = i.toLong(), updatedAt = now)
        })
    }

    suspend fun updateNote(note: Note) = db.notes().update(note.copy(updatedAt = now()))

    suspend fun deleteNote(note: Note) = db.notes().delete(note)

    suspend fun deleteNoteIds(ids: List<Long>) = db.notes().deleteIds(ids)

    /** Undo: puts a deleted note back, as a new change so it wins over the deletion everywhere. */
    suspend fun restoreNote(note: Note) = db.notes().insert(note.copy(updatedAt = 0))

    suspend fun markDone(note: Note, outcome: Int) = db.notes().update(
        note.copy(status = Status.DONE, outcome = outcome, doneAt = note.doneAt ?: System.currentTimeMillis(), updatedAt = now())
    )

    suspend fun reopen(note: Note) = db.notes().update(
        note.copy(status = Status.OPEN, outcome = Outcome.NONE, doneAt = null, updatedAt = now())
    )

    // Categories (each belongs to one person)

    suspend fun addCollection(personId: Long, name: String, occasion: String, occasionDay: Long?, yearly: Boolean): Long {
        val mine = db.collections().forPerson(personId)
        val usedSlots = mine.map { it.slot }.toSet()
        val slot = (0 until Slot.COUNT).firstOrNull { it !in usedSlots } ?: (mine.size % Slot.COUNT)
        return db.collections().insert(
            NoteCollection(
                personId = personId, name = name.trim(), kind = Kind.CUSTOM, slot = slot,
                sortOrder = (mine.maxOfOrNull { it.sortOrder } ?: 0) + 1,
                occasion = occasion, occasionDay = occasionDay, yearly = yearly,
            )
        )
    }

    suspend fun updateCollection(collection: NoteCollection) = db.collections().update(collection.copy(name = collection.name.trim(), updatedAt = now()))

    /** Moves a category one place earlier (-1) or later (+1) among that person's categories. */
    suspend fun moveCollection(collection: NoteCollection, delta: Int) {
        val pid = collection.personId ?: return
        val mine = db.collections().forPerson(pid).toMutableList()
        val i = mine.indexOfFirst { it.id == collection.id }
        val j = i + delta
        if (i < 0 || j !in mine.indices) return
        mine.add(j, mine.removeAt(i))
        db.collections().updateAll(mine.mapIndexed { index, c -> c.copy(sortOrder = index, updatedAt = now()) })
    }

    /** Drag and drop: puts a category in [target]'s place among that person's categories. */
    suspend fun placeCollection(id: Long, target: Long) {
        val t = db.collections().all().firstOrNull { it.id == target } ?: return
        val pid = t.personId ?: return
        val mine = db.collections().forPerson(pid)
        if (mine.none { it.id == id }) return
        val now = now()
        db.collections().updateAll(place(mine, { it.id }, id, target).mapIndexedNotNull { i, c ->
            if (c.sortOrder == i) null else c.copy(sortOrder = i, updatedAt = now)
        })
    }

    /** [list] with [id] moved to where [target] is. */
    private fun <T> place(list: List<T>, key: (T) -> Long, id: Long, target: Long): List<T> {
        val from = list.indexOfFirst { key(it) == id }
        val to = list.indexOfFirst { key(it) == target }
        if (from < 0 || to < 0) return list
        return list.toMutableList().apply { add(to, removeAt(from)) }
    }

    /** Removes a category. Its notes move to another of the person's categories so nothing is lost. */
    suspend fun deleteCollection(collection: NoteCollection) {
        val pid = collection.personId
        val others = if (pid != null) db.collections().forPerson(pid).filter { it.id != collection.id } else emptyList()
        val target = others.firstOrNull { it.kind == Kind.NOTE } ?: others.firstOrNull()
        if (target != null) db.notes().moveAll(collection.id, target.id)
        else db.notes().all().filter { it.collectionId == collection.id }.forEach { db.notes().delete(it) }
        db.collections().delete(collection)
    }
}
