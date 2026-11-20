package app.memi.data


/*
 * What the screens show. Every record has two ids: `id`, the row number on this phone, and `uid`,
 * the same on every device, so copies of a Memi can be merged item by item. `updatedAt` is when
 * the record last changed anywhere; the newest change wins. memi-engine (the shared Rust engine,
 * see Repository) fills both in and keeps them up to date.
 */

data class Person(
    val id: Long = 0,
    val uid: String = "",
    val updatedAt: Long = 0,
    val name: String,
    val relationship: String = "",
    /** Absolute path of the app's private copy of the photo, or null. */
    val photoPath: String? = null,
    val birthdayMonth: Int? = null, // 1..12
    val birthdayDay: Int? = null,   // 1..31
    val birthYear: Int? = null,
    /** Index into the palette's placeholder tones, used when there's no photo. */
    val toneIndex: Int = 0,
    /** Pinned people come first on Home. */
    val pinned: Boolean = false,
    /** Your own order (lower comes first); ties fall back to name. */
    val sortOrder: Int = 0,
    val createdAt: Long = System.currentTimeMillis(),
) {
    val firstName: String get() = name.trim().substringBefore(' ')
    val hasBirthday: Boolean get() = birthdayMonth != null && birthdayDay != null
}

/** The colour slot a collection uses. The first three match the starter collections. */
object Slot {
    const val GIFT = 0
    const val PLAN = 1
    const val NOTE = 2
    const val EXTRA_A = 3
    const val EXTRA_B = 4
    const val COUNT = 5
}

/** Where a category came from: one of the three starters, or one you made. Only affects defaults. */
object Kind {
    const val GIFT = "GIFT"
    const val PLAN = "PLAN"
    const val NOTE = "NOTE"
    const val CUSTOM = "CUSTOM"
}

/** What a category is building up to, if anything. */
object Occasion {
    const val NONE = "NONE"
    /** The person's birthday, every year. */
    const val BIRTHDAY = "BIRTHDAY"
    /** A specific date ([NoteCollection.occasionDay]), once or every year. */
    const val DATE = "DATE"
}

/** A category belongs to one person, so everyone can have their own set. */
data class NoteCollection(
    val id: Long = 0,
    val uid: String = "",
    val updatedAt: Long = 0,
    val personId: Long? = null,
    val name: String,
    val kind: String,
    val slot: Int,
    val sortOrder: Int,
    val occasion: String = Occasion.NONE,
    /** For [Occasion.DATE]: the date as days since 1970-01-01. */
    val occasionDay: Long? = null,
    /** For [Occasion.DATE]: repeat on the same day every year (anniversaries). */
    val yearly: Boolean = false,
)

object Status {
    const val OPEN = 0
    const val DONE = 1
}

object Outcome {
    const val NONE = 0
    const val HIT = 1
    const val MISS = 2
}

data class Note(
    val id: Long = 0,
    val uid: String = "",
    val updatedAt: Long = 0,
    val personId: Long,
    val collectionId: Long,
    val text: String,
    /** When this was last mentioned. Re-mentions move it forward. */
    val mentionedAt: Long = System.currentTimeMillis(),
    val firstMentionedAt: Long = mentionedAt,
    val mentionCount: Int = 1,
    /** Optional deadline, e.g. the birthday a gift idea is for, or "by Friday". */
    val dueAt: Long? = null,
    val status: Int = Status.OPEN,
    val outcome: Int = Outcome.NONE,
    val doneAt: Long? = null,
    val createdAt: Long = System.currentTimeMillis(),
    /** Your own order within the category (lower comes first). New notes go to the top. */
    val sortOrder: Long = -mentionedAt,
)
