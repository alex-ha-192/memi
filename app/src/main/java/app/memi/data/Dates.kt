package app.memi.data

import java.time.DayOfWeek
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.TextStyle
import java.time.temporal.ChronoUnit
import java.util.Locale

object Dates {
    val zone: ZoneId get() = ZoneId.systemDefault()

    fun today(): LocalDate = LocalDate.now(zone)

    fun toDate(millis: Long): LocalDate = Instant.ofEpochMilli(millis).atZone(zone).toLocalDate()

    /** End of the given day, so "due Sunday" stays due all of Sunday. */
    fun endOfDay(date: LocalDate): Long =
        date.plusDays(1).atStartOfDay(zone).toInstant().toEpochMilli() - 1

    fun nextBirthday(person: Person, from: LocalDate = today()): LocalDate? {
        val m = person.birthdayMonth ?: return null
        val d = person.birthdayDay ?: return null
        fun on(year: Int): LocalDate {
            val month = java.time.YearMonth.of(year, m)
            return month.atDay(d.coerceAtMost(month.lengthOfMonth()))
        }
        val thisYear = on(from.year)
        return if (thisYear.isBefore(from)) on(from.year + 1) else thisYear
    }

    /** The next day a category is building up to: their birthday, a one-off date, or an anniversary. */
    fun nextOccasion(c: NoteCollection, person: Person, from: LocalDate = today()): LocalDate? = when (c.occasion) {
        Occasion.BIRTHDAY -> nextBirthday(person, from)
        Occasion.DATE -> {
            val d = c.occasionDay?.let { LocalDate.ofEpochDay(it) }
            when {
                d == null -> null
                c.yearly -> {
                    fun on(year: Int): LocalDate {
                        val month = java.time.YearMonth.of(year, d.monthValue)
                        return month.atDay(d.dayOfMonth.coerceAtMost(month.lengthOfMonth()))
                    }
                    val thisYear = on(from.year)
                    if (thisYear.isBefore(from)) on(from.year + 1) else thisYear
                }
                d.isBefore(from) -> null
                else -> d
            }
        }
        else -> null
    }

    /** The age someone turns on their next birthday, if their birth year is known. */
    fun turningAge(person: Person, from: LocalDate = today()): Int? {
        val year = person.birthYear ?: return null
        val next = nextBirthday(person, from) ?: return null
        return next.year - year
    }

    fun daysUntil(date: LocalDate, from: LocalDate = today()): Long = ChronoUnit.DAYS.between(from, date)

    /** "Today", "Tomorrow", "Sunday" (within a week) or "22 Nov". */
    fun dayLabel(date: LocalDate, from: LocalDate = today()): String {
        val days = daysUntil(date, from)
        return when {
            days == 0L -> "Today"
            days == 1L -> "Tomorrow"
            days in 2..6 -> date.dayOfWeek.getDisplayName(TextStyle.FULL, Locale.getDefault())
            else -> shortDate(date)
        }
    }

    /** Short form for tags: "Today", "Thu", "22 Nov". */
    fun shortDayLabel(date: LocalDate, from: LocalDate = today()): String {
        val days = daysUntil(date, from)
        return when {
            days < 0 -> "Overdue"
            days == 0L -> "Today"
            days == 1L -> "Tmrw"
            days in 2..6 -> date.dayOfWeek.getDisplayName(TextStyle.SHORT, Locale.getDefault())
            else -> shortDate(date)
        }
    }

    fun inDays(date: LocalDate, from: LocalDate = today()): String {
        val days = daysUntil(date, from)
        return when (days) {
            0L -> "today"
            1L -> "tomorrow"
            else -> "$days days"
        }
    }

    fun shortDate(date: LocalDate): String =
        "${date.dayOfMonth} ${date.month.getDisplayName(TextStyle.SHORT, Locale.getDefault())}"

    /** "Jun", or "Nov ’25" for an earlier year. */
    fun monthLabel(millis: Long, from: LocalDate = today()): String {
        val d = toDate(millis)
        val month = d.month.getDisplayName(TextStyle.SHORT, Locale.getDefault())
        return if (d.year == from.year) month else "$month ’${(d.year % 100).toString().padStart(2, '0')}"
    }

    /** "today", "yesterday", "2 days ago", "last week", "3 weeks ago", "2 months ago", "in August". */
    fun ago(millis: Long, now: LocalDate = today()): String {
        val d = toDate(millis)
        val days = ChronoUnit.DAYS.between(d, now)
        return when {
            days <= 0 -> "today"
            days == 1L -> "yesterday"
            days < 7 -> "$days days ago"
            days < 14 -> "last week"
            days < 31 -> "${days / 7} weeks ago"
            days < 60 -> "last month"
            days < 330 -> "${days / 30} months ago"
            else -> "in ${d.month.getDisplayName(TextStyle.FULL, Locale.getDefault())} ${d.year}"
        }
    }

    /** "2d", "6d", "2w" for compact tags. */
    fun agoShort(millis: Long, now: LocalDate = today()): String {
        val days = ChronoUnit.DAYS.between(toDate(millis), now)
        return when {
            days <= 0 -> "today"
            days < 7 -> "${days}d"
            else -> "${days / 7}w"
        }
    }

    fun nextWeekday(day: DayOfWeek, from: LocalDate = today(), allowToday: Boolean = false): LocalDate {
        var d = if (allowToday) from else from.plusDays(1)
        while (d.dayOfWeek != day) d = d.plusDays(1)
        return d
    }
}
