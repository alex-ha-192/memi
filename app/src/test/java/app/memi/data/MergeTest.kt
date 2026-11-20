package app.memi.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/** The same cases as core/src/merge.rs, so the phone and the computer merge identically. */
class MergeTest {
    private fun person(uid: String, name: String, at: Long) = Person(uid = uid, name = name, updatedAt = at, createdAt = 1)
    private fun col(uid: String, person: String, kind: String, at: Long) =
        CollectionRec(NoteCollection(uid = uid, name = uid, kind = kind, slot = 0, sortOrder = 0, updatedAt = at), person)
    private fun note(uid: String, person: String, col: String, text: String, at: Long) =
        NoteRec(Note(uid = uid, personId = 0, collectionId = 0, text = text, mentionedAt = at, createdAt = at, updatedAt = at), person, col)

    // Close to the made-up edit times below, so no deletion counts as old.
    private val now = 100L

    @Test fun editsOnBothSidesAreKept() {
        val base = person("p", "Maya", 1)
        val local = Snapshot(listOf(base), listOf(col("c", "p", Kind.NOTE, 1)), listOf(note("a", "p", "c", "Added on the phone", 10)))
        val remote = Snapshot(listOf(base.copy(name = "Maya Okafor", updatedAt = 5)), listOf(col("c", "p", Kind.NOTE, 1)), listOf(note("b", "p", "c", "Added on the computer", 11)))
        val m = Merge.merge(local, remote, now)
        assertEquals(2, m.snapshot.notes.size)
        assertEquals("the newer edit wins", "Maya Okafor", m.snapshot.people[0].name)
        assertTrue(m.localChanged && m.remoteChanged)
        assertTrue("p" in m.peopleFromRemote)
    }

    @Test fun deletionsSpreadButLaterEditsWin() {
        val p = person("p", "Maya", 1)
        val c = col("c", "p", Kind.NOTE, 1)
        val local = Snapshot(listOf(p), listOf(c), listOf(note("a", "p", "c", "Old", 5), note("b", "p", "c", "Edited after", 20)))
        val remote = Snapshot(listOf(p), listOf(c), emptyList(), listOf(Tombstone("a", 10), Tombstone("b", 10)))
        val m = Merge.merge(local, remote, now)
        assertEquals(listOf("Edited after"), m.snapshot.notes.map { it.n.text })
        assertEquals("the tombstone for the survivor is dropped", 1, m.snapshot.tombstones.size)
    }

    @Test fun removingAPersonTakesTheirThingsAndOrphansMove() {
        val local = Snapshot(
            listOf(person("p", "Maya", 1), person("q", "Sam", 1)),
            listOf(col("gone", "q", Kind.GIFT, 1), col("notes", "q", Kind.NOTE, 1), col("c", "p", Kind.NOTE, 1)),
            listOf(note("x", "p", "c", "Maya's", 1), note("y", "q", "gone", "Sam's", 1)),
        )
        val remote = Snapshot(tombstones = listOf(Tombstone("p", 5), Tombstone("gone", 5)))
        val m = Merge.merge(local, remote, now)
        assertEquals(1, m.snapshot.people.size)
        assertEquals(1, m.snapshot.notes.size)
        assertEquals("moved to Sam's Notes", "notes", m.snapshot.notes[0].collectionUid)
    }

    @Test fun identicalCopiesNeedNothing() {
        val s = Snapshot(people = listOf(person("p", "Maya", 3)))
        val m = Merge.merge(s, s, now)
        assertFalse(m.localChanged || m.remoteChanged)
    }

    @Test fun mergingIsTheSameFromBothSides() {
        val a = Snapshot(people = listOf(person("p", "A", 2), person("r", "R", 1)))
        val b = Snapshot(people = listOf(person("p", "B", 3)), tombstones = listOf(Tombstone("r", 9)))
        assertEquals(Merge.merge(a, b, now).snapshot, Merge.merge(b, a, now).snapshot)
    }

    @Test fun oldDeletionsAreForgotten() {
        val s = Snapshot(tombstones = listOf(Tombstone("x", 0)))
        val m = Merge.merge(s, Snapshot(), Merge.TOMBSTONE_DAYS * 24 * 60 * 60 * 1000 + 1)
        assertTrue(m.snapshot.tombstones.isEmpty())
    }
}
