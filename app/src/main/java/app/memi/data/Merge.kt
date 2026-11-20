package app.memi.data

/**
 * A whole Memi in a form that can be compared across devices: records refer to their parents by
 * uid rather than by this phone's row ids.
 */
data class Snapshot(
    val people: List<Person> = emptyList(),
    val collections: List<CollectionRec> = emptyList(),
    val notes: List<NoteRec> = emptyList(),
    val tombstones: List<Tombstone> = emptyList(),
)

data class CollectionRec(val c: NoteCollection, val personUid: String?)
data class NoteRec(val n: Note, val personUid: String, val collectionUid: String)

/**
 * Combining two copies of a Memi (this device's and the one in your Google Drive) item by item.
 * Exactly the same rules as the desktop and web apps (core/src/merge.rs):
 *
 * - Every person, category and note is matched by its uid. Where both copies have it, the one
 *   changed most recently wins; ties keep this device's version.
 * - A deletion beats every version of the item from before it was deleted. An item edited after it
 *   was deleted somewhere else comes back.
 * - Afterwards, anything whose person is gone is dropped, and a note whose category is gone moves
 *   to the person's Notes category (or else their first).
 */
object Merge {
    /** Deletions older than this are forgotten. */
    const val TOMBSTONE_DAYS = 180L

    class Result(
        val snapshot: Snapshot,
        /** This device's copy needs updating to match. */
        val localChanged: Boolean,
        /** The copy in Drive needs updating to match. */
        val remoteChanged: Boolean,
        /** People whose version came from Drive (their photo comes from there too). */
        val peopleFromRemote: Set<String>,
    )

    private fun <T> pick(local: List<T>, remote: List<T>, uid: (T) -> String, updated: (T) -> Long): Pair<List<T>, Set<String>> {
        val out = LinkedHashMap<String, T>()
        local.forEach { out[uid(it)] = it }
        val fromRemote = HashSet<String>()
        for (r in remote) {
            val mine = out[uid(r)]
            if (mine != null && updated(mine) >= updated(r)) continue
            out[uid(r)] = r
            fromRemote += uid(r)
        }
        return out.values.sortedBy(uid) to fromRemote
    }

    fun merge(local: Snapshot, remote: Snapshot, now: Long = System.currentTimeMillis()): Result {
        val tombs = HashMap<String, Long>()
        for (t in local.tombstones + remote.tombstones) tombs[t.uid] = maxOf(tombs[t.uid] ?: t.deletedAt, t.deletedAt)
        val cutoff = now - TOMBSTONE_DAYS * 24 * 60 * 60 * 1000
        tombs.entries.removeAll { it.value < cutoff }
        fun alive(uid: String, updated: Long) = tombs[uid]?.let { updated > it } ?: true

        val (allPeople, fromRemote) = pick(local.people, remote.people, { it.uid }, { it.updatedAt })
        val people = allPeople.filter { alive(it.uid, it.updatedAt) }
        val personUids = people.map { it.uid }.toSet()

        val cols = pick(local.collections, remote.collections, { it.c.uid }, { it.c.updatedAt }).first
            .filter { alive(it.c.uid, it.c.updatedAt) && it.personUid != null && it.personUid in personUids }
        val colUids = cols.map { it.c.uid }.toSet()

        fun fallback(person: String): String? {
            val mine = cols.filter { it.personUid == person }.sortedWith(compareBy({ it.c.sortOrder }, { it.c.uid }))
            return (mine.firstOrNull { it.c.kind == Kind.NOTE } ?: mine.firstOrNull())?.c?.uid
        }
        val notes = pick(local.notes, remote.notes, { it.n.uid }, { it.n.updatedAt }).first
            .filter { alive(it.n.uid, it.n.updatedAt) && it.personUid in personUids }
            .mapNotNull { if (it.collectionUid in colUids) it else fallback(it.personUid)?.let { c -> it.copy(collectionUid = c) } }

        val survivors = people.map { it.uid }.toSet() + colUids + notes.map { it.n.uid }
        val tombstones = tombs.filterKeys { it !in survivors }.map { (uid, at) -> Tombstone(uid, at) }.sortedBy { it.uid }

        val merged = Snapshot(people, cols, notes, tombstones)
        return Result(
            snapshot = merged,
            localChanged = !same(merged, local),
            remoteChanged = !same(merged, remote),
            peopleFromRemote = fromRemote.filter { it in personUids }.toSet(),
        )
    }

    private fun same(a: Snapshot, b: Snapshot): Boolean {
        fun v(s: Snapshot) = listOf(
            s.people.associate { it.uid to it.updatedAt },
            s.collections.associate { it.c.uid to it.c.updatedAt },
            s.notes.associate { it.n.uid to it.n.updatedAt },
            s.notes.associate { it.n.uid to it.collectionUid },
            s.tombstones.associate { it.uid to it.deletedAt },
        )
        return v(a) == v(b)
    }
}
