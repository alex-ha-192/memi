package app.memi

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.input.key.KeyEventType
import androidx.compose.ui.input.key.key
import androidx.compose.ui.input.key.type
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Window
import androidx.compose.ui.window.WindowPosition
import androidx.compose.ui.window.application
import androidx.compose.ui.window.rememberWindowState
import androidx.datastore.preferences.core.PreferenceDataStoreFactory
import androidx.room.Room
import androidx.sqlite.driver.bundled.BundledSQLiteDriver
import app.memi.data.AppDatabase
import app.memi.data.Repository
import app.memi.data.ThemePrefs
import app.memi.platform.AppDirs
import app.memi.platform.DesktopBack
import app.memi.sync.DesktopSyncPlatform
import app.memi.sync.SyncManager
import app.memi.ui.AppViewModel
import app.memi.ui.MemiRoot
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeoutOrNull
import java.awt.Dimension
import java.io.File

/** Memi for Windows and Linux: the same app as on Android, in a window. */
@Suppress("DEPRECATION")
fun main() {
    val dataDir = AppDirs.data
    val db = Room.databaseBuilder<AppDatabase>(name = File(dataDir, "memi.db").absolutePath)
        .setDriver(BundledSQLiteDriver())
        .setQueryCoroutineContext(Dispatchers.IO)
        .build()
    val repository = Repository(db)
    val themePrefs = ThemePrefs(PreferenceDataStoreFactory.create(produceFile = { File(dataDir, "settings.preferences_pb") }))
    val background = CoroutineScope(SupervisorJob() + Dispatchers.Default)
    val sync = SyncManager(DesktopSyncPlatform(), db, background)

    // Any change to people, categories or notes queues an encrypted upload.
    background.launch {
        combine(repository.people, repository.collections, repository.notes) { _, _, _ -> }
            .drop(1)
            .collect { sync.onLocalChange() }
    }

    application {
        val vm = remember { AppViewModel(repository, themePrefs) }
        val scope = rememberCoroutineScope()
        var closing by remember { mutableStateOf(false) }
        val state = rememberWindowState(width = 1180.dp, height = 820.dp, position = WindowPosition(Alignment.Center))

        Window(
            onCloseRequest = {
                // Hide straight away, send anything not yet synced (briefly), then quit.
                if (!closing) {
                    closing = true
                    scope.launch {
                        withContext(Dispatchers.IO) { withTimeoutOrNull(8_000) { sync.flush() } }
                        exitApplication()
                    }
                }
            },
            visible = !closing,
            state = state,
            title = "Memi",
            icon = painterResource("memi_icon.png"),
            // Escape works like Android's back: closes the sheet on top, then search, then the theme panel.
            onKeyEvent = { e -> e.key == Key.Escape && e.type == KeyEventType.KeyDown && DesktopBack.back() },
        ) {
            LaunchedEffect(Unit) { window.minimumSize = Dimension(420, 640) }
            // Coming back to the window brings in changes from your phone.
            val focused = LocalWindowInfo.current.isWindowFocused
            LaunchedEffect(focused) { if (focused) sync.onForeground() }
            MemiRoot(vm, sync)
        }
    }
}
