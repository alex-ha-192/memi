package app.memi

import android.app.Application
import androidx.datastore.preferences.preferencesDataStore
import app.memi.data.Repository
import app.memi.data.ThemePrefs
import app.memi.platform.AndroidDatabase
import app.memi.platform.PlatformPhotos
import app.memi.sync.AndroidSyncPlatform
import app.memi.sync.SyncManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import app.memi.platform.BirthdayReminders

private val android.content.Context.settingsStore by preferencesDataStore(name = "settings")

class MemiApp : Application() {
    lateinit var repository: Repository
        private set
    lateinit var themePrefs: ThemePrefs
        private set
    lateinit var sync: SyncManager
        private set

    private val appScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

    override fun onCreate() {
        super.onCreate()
        PlatformPhotos.init(this)
        val db = AndroidDatabase.create(this)
        repository = Repository(db)
        themePrefs = ThemePrefs(settingsStore)
        sync = SyncManager(AndroidSyncPlatform(this), db, appScope)

        // Birthday reminders (off unless turned on): keep the daily check scheduled.
        appScope.launch {
            BirthdayReminders.schedule(this@MemiApp, themePrefs.birthdayReminders.first().isNotEmpty())
        }

        // Any change to people, categories or notes queues an encrypted upload.
        appScope.launch {
            combine(repository.people, repository.collections, repository.notes) { _, _, _ -> }
                .drop(1)
                .collect { sync.onLocalChange() }
        }
    }
}
