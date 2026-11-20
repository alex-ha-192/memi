package app.memi.ui

import app.memi.data.Dates
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Occasion
import app.memi.data.Person
import app.memi.data.Status
import java.time.LocalDate
import java.time.temporal.ChronoUnit

/** Derived views of the data: what's fresh, what's due, what's coming up next. */
object Insights {
    private const val FRESH_DAYS = 14L
    private const val SOON_DAYS = 14L
    private const val UPCOMING_DAYS = 30L

    data class FreshItem(
        val note: Note,
        val person: Person,
        val collection: NoteCollection?,
        /** Set when something is due soon; otherwise the item is simply new. */
        val due: LocalDate?,
    )

    /**
     * Things added recently, plus anything with a date coming up: its own deadline, or the date
     * its category builds up to (a birthday, an anniversary, a trip). Soonest first, then newest.
     */
    fun stillFresh(
        notes: List<Note>,
        people: List<Person>,
        collections: List<NoteCollection>,
        today: LocalDate = Dates.today(),
    ): List<FreshItem> {
        val byId = people.associateBy { it.id }
        val cols = collections.associateBy { it.id }
        return notes.asSequence()
            .filter { it.status == Status.OPEN }
            .mapNotNull { n ->
                val person = byId[n.personId] ?: return@mapNotNull null
                val col = cols[n.collectionId]
                val due = effectiveDue(n, person, col, today)
                val dueSoon = due != null && ChronoUnit.DAYS.between(today, due) <= SOON_DAYS
                val fresh = ChronoUnit.DAYS.between(Dates.toDate(n.mentionedAt), today) <= FRESH_DAYS
                if (!dueSoon && !fresh) return@mapNotNull null
                FreshItem(n, person, col, if (dueSoon) due else null)
            }
            .sortedWith(compareBy<FreshItem>({ it.due == null }, { it.due }).thenByDescending { it.note.mentionedAt })
            .toList()
    }

    /** A note's own deadline, or else the next date its category is building up to. */
    fun effectiveDue(note: Note, person: Person, collection: NoteCollection?, today: LocalDate = Dates.today()): LocalDate? {
        note.dueAt?.let { return Dates.toDate(it) }
        return collection?.let { Dates.nextOccasion(it, person, today) }
    }

    /** One thing coming up, for the Coming up tile on Home. */
    data class Upcoming(
        val person: Person,
        val date: LocalDate,
        val title: String,
        val open: Int,
        /** The category to open when tapped, if there is one. */
        val collection: NoteCollection?,
        /** Set when this is a dated plan or note. */
        val note: Note? = null,
        val isBirthday: Boolean = false,
    )

    /**
     * The next few things coming up, soonest first, however far away: birthdays, dated categories
     * (an anniversary, a trip) and open plans or notes with their own date. Same as the desktop's
     * (core/src/insights.rs `next_events`).
     */
    fun nextEvents(
        people: List<Person>,
        notes: List<Note>,
        collections: List<NoteCollection>,
        count: Int = 3,
        today: LocalDate = Dates.today(),
    ): List<Upcoming> {
        val openByCollection = notes.filter { it.status == Status.OPEN }.groupingBy { it.collectionId }.eachCount()
        val byId = people.associateBy { it.id }
        val colById = collections.associateBy { it.id }
        val all = mutableListOf<Upcoming>()
        for (p in people) {
            val mine = collections.filter { it.personId == p.id }
            Dates.nextBirthday(p, today)?.let { d ->
                val birthdayCats = mine.filter { it.occasion == Occasion.BIRTHDAY }
                val turning = Dates.turningAge(p, today)
                all += Upcoming(
                    person = p, date = d,
                    title = if (turning != null) "${p.firstName} turns $turning" else "${p.firstName}'s birthday",
                    open = birthdayCats.sumOf { openByCollection[it.id] ?: 0 },
                    collection = birthdayCats.firstOrNull(),
                    isBirthday = true,
                )
            }
            mine.filter { it.occasion == Occasion.DATE }.forEach { c ->
                Dates.nextOccasion(c, p, today)?.let { d ->
                    all += Upcoming(p, d, "${c.name} · ${p.firstName}", openByCollection[c.id] ?: 0, c)
                }
            }
        }
        for (n in notes) {
            if (n.status != Status.OPEN) continue
            val due = n.dueAt ?: continue
            val p = byId[n.personId] ?: continue
            val d = Dates.toDate(due)
            if (d < today) continue
            all += Upcoming(p, d, n.text, 0, colById[n.collectionId], note = n)
        }
        return all.sortedWith(compareBy({ it.date }, { !it.isBirthday }, { it.note != null })).take(count)
    }

    fun openCount(person: Person, notes: List<Note>): Int =
        notes.count { it.personId == person.id && it.status == Status.OPEN }
}
