package app.memi.data

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class BackupTest {
    /** Must match core/src/model.rs legacy_uid (tested there with the same values). */
    @Test fun legacyUidsMatchTheDesktop() {
        assertEquals("a1e4984a7e39eea1336fa23191d15348", LegacyUid.person(3, 1700000000000))
        assertEquals("4fd968f882732d432b8ffa516af1ea10", LegacyUid.collection(7, 3, 1700000000000))
        assertEquals("6e53e5417e60db96d0d6bda4d1d200a5", LegacyUid.collection(9, null, null))
        assertEquals("ed39b1b44db4f7dc2b2ecabca8a47f90", LegacyUid.note(12, 1700000000123))
    }

    @Test fun roundTrip() {
        val p = Person(id = 1, uid = "p1", updatedAt = 5, name = "Maya", birthdayMonth = 11, birthdayDay = 26, createdAt = 1)
        val c = NoteCollection(id = 2, uid = "c1", updatedAt = 6, personId = 1, name = "Gift ideas", kind = Kind.GIFT, slot = 0, sortOrder = 0)
        val n = Note(id = 3, uid = "n1", updatedAt = 7, personId = 1, collectionId = 2, text = "Blue mug", mentionedAt = 50, createdAt = 50, sortOrder = -9)
        val s = Snapshot(listOf(p), listOf(CollectionRec(c, "p1")), listOf(NoteRec(n, "p1", "c1")), listOf(Tombstone("gone", 4)))
        val doc = Backup.import(Backup.export(s))
        assertEquals(s, doc.snapshot)
        assertTrue(!doc.legacy)
    }

    /** A Memi 0.5 backup: no uids, parents by row id. */
    @Test fun formatOneGetsLegacyUids() {
        val json = """{"format":1,"people":[{"id":3,"name":"Maya","createdAt":1700000000000}],
            "collections":[{"id":7,"personId":3,"name":"Notes","kind":"NOTE","slot":2,"sortOrder":2}],
            "notes":[{"id":12,"personId":3,"collectionId":7,"text":"Hi","mentionedAt":1700000000123,"createdAt":1700000000123}]}"""
        val doc = Backup.import(json.toByteArray())
        assertTrue(doc.legacy)
        assertEquals("a1e4984a7e39eea1336fa23191d15348", doc.snapshot.people[0].uid)
        assertEquals("4fd968f882732d432b8ffa516af1ea10", doc.snapshot.collections[0].c.uid)
        assertEquals("a1e4984a7e39eea1336fa23191d15348", doc.snapshot.notes[0].personUid)
        assertEquals(-1700000000123, doc.snapshot.notes[0].n.sortOrder)
    }

    @Test(expected = Backup.TooNew::class) fun newerFormatsAreRefused() {
        Backup.import("""{"format":99}""".toByteArray())
    }
}
