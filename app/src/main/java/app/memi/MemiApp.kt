package app.memi

import android.app.Application
import androidx.datastore.preferences.preferencesDataStore
import app.memi.data.Repository
import app.memi.data.ThemePrefs
import app.memi.engine.Engine
import app.memi.engine.SyncState
import app.memi.platform.BirthdayReminders
import app.memi.platform.PlatformPhotos
import app.memi.sync.AndroidSyncPlatform
import app.memi.sync.EngineHost
import app.memi.sync.SettingsStore
import app.memi.sync.SyncManager
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import java.io.File

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
        val platform = AndroidSyncPlatform(this)
        // memi-engine opens the same memi.db earlier versions kept (Room's), upgrading it in place.
        // The first time, it keeps a copy exactly as it was, in case anything needs recovering.
        val db = getDatabasePath("memi.db").apply { parentFile?.mkdirs() }
        val engine = Engine(
            dbPath = db.path,
            untouchedCopy = File(db.parentFile, "memi-0.6.0.db").path,
            photoDir = PlatformPhotos.dir.path,
            syncPath = File(filesDir, "sync.json").path,
            legacySync = legacySync(platform.settings),
            host = EngineHost(platform),
        )
        repository = Repository(engine)
        themePrefs = ThemePrefs(settingsStore)
        sync = SyncManager(platform, engine, repository, appScope)

        // Birthday reminders (off unless turned on): keep the daily check scheduled.
        appScope.launch {
            BirthdayReminders.schedule(this@MemiApp, themePrefs.birthdayReminders.first().isNotEmpty())
        }

        // Any change you make queues an encrypted upload.
        repository.onEdit = { sync.onLocalChange() }
    }

    /** Sync's state as versions before 0.6.1 kept it (in preferences); the engine keeps it in sync.json now. */
    private fun legacySync(prefs: SettingsStore): SyncState? {
        if (!prefs.getBoolean("enabled") && prefs.getString("account") == null) return null
        return SyncState(
            enabled = prefs.getBoolean("enabled"),
            account = prefs.getString("account"),
            remoteVersion = prefs.getString("remoteVersion"),
            dirty = prefs.getBoolean("dirty"),
            lastSync = prefs.getLong("lastSync").takeIf { it > 0 },
        )
    }
}
