package app.memi.platform

import android.app.AlarmManager
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import app.memi.MainActivity
import app.memi.MemiApp
import app.memi.R
import app.memi.ui.Insights
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId

/**
 * Optional birthday reminders: once a day, around 9 in the morning, Memi checks for birthdays
 * on the days you picked (on the day, the day before, a week before) and shows a notification.
 * Everything happens on the phone; nothing is sent anywhere.
 */
object BirthdayReminders {
    private const val CHANNEL = "birthdays"
    private const val HOUR = 9

    /** Starts or stops the daily check to match the setting. Safe to call repeatedly. */
    fun schedule(context: Context, on: Boolean) {
        val alarms = context.getSystemService(AlarmManager::class.java) ?: return
        val pending = PendingIntent.getBroadcast(
            context, 0, Intent(context, ReminderReceiver::class.java),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        alarms.cancel(pending)
        if (!on) return
        val zone = ZoneId.systemDefault()
        var next = LocalDate.now(zone).atTime(LocalTime.of(HOUR, 0)).atZone(zone)
        if (!next.isAfter(java.time.ZonedDateTime.now(zone))) next = next.plusDays(1)
        // Inexact, so Android can batch it with other wake-ups; a few minutes either way is fine.
        alarms.setInexactRepeating(AlarmManager.RTC_WAKEUP, next.toInstant().toEpochMilli(), AlarmManager.INTERVAL_DAY, pending)
    }

    /** Shows any reminders due today that haven't been shown yet. */
    suspend fun check(context: Context) {
        val app = context.applicationContext as MemiApp
        val days = app.themePrefs.birthdayReminders.first()
        if (days.isEmpty()) return
        // Not before 9: a reminder at 2 am (after a restart, say) would be unwelcome.
        if (LocalTime.now().hour < HOUR) return
        val repo = app.repository
        val due = Insights.remindersDue(repo.people.first(), repo.notes.first(), repo.collections.first(), days)
        val shown = app.themePrefs.reminded.first()
        val fresh = due.filter { it.key !in shown }
        if (fresh.isEmpty() || !canNotify(context)) return
        ensureChannel(context)
        val open = PendingIntent.getActivity(
            context, 0, Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_IMMUTABLE,
        )
        val nm = NotificationManagerCompat.from(context)
        for (r in fresh) {
            val n = NotificationCompat.Builder(context, CHANNEL)
                .setSmallIcon(R.drawable.ic_notification)
                .setContentTitle(r.title)
                .setContentText(r.body)
                .setContentIntent(open)
                .setAutoCancel(true)
                .setCategory(NotificationCompat.CATEGORY_REMINDER)
                .build()
            runCatching { nm.notify(r.key.hashCode(), n) }
        }
        app.themePrefs.addReminded(fresh.map { it.key })
    }

    fun canNotify(context: Context): Boolean =
        Build.VERSION.SDK_INT < 33 ||
            ContextCompat.checkSelfPermission(context, android.Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED

    private fun ensureChannel(context: Context) {
        val nm = context.getSystemService(NotificationManager::class.java) ?: return
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Birthday reminders", NotificationManager.IMPORTANCE_DEFAULT))
    }
}

/** Woken by the daily alarm, and after the phone restarts (alarms don't survive a restart). */
class ReminderReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val result = goAsync()
        CoroutineScope(Dispatchers.Default).launch {
            try {
                val app = context.applicationContext as MemiApp
                if (intent.action == Intent.ACTION_BOOT_COMPLETED || intent.action == Intent.ACTION_MY_PACKAGE_REPLACED) {
                    BirthdayReminders.schedule(context, app.themePrefs.birthdayReminders.first().isNotEmpty())
                }
                BirthdayReminders.check(context)
            } finally {
                result.finish()
            }
        }
    }
}
