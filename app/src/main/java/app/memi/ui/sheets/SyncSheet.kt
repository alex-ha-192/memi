package app.memi.ui.sheets

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Dns
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.foundation.clickable
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.memi.platform.rememberGoogleSignIn
import app.memi.ui.LocalSync
import app.memi.sync.SyncManager
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import app.memi.platform.rememberQrScanner

/** Sync settings: pick a place once (Google Drive, WebDAV or a folder), and Memi keeps an encrypted copy there from then on. */
@Composable
fun SyncSheet(onDismiss: () -> Unit) {
    val sync = LocalSync.current
    val state by sync.state.collectAsStateWithLifecycle()
    val colors = Bento.colors
    var signInError by remember { mutableStateOf<String?>(null) }

    // Google's own account picker.
    val signIn = rememberGoogleSignIn(
        onToken = { token -> signInError = null; sync.launch { connect(token) } },
        onError = { message -> signInError = message },
    )
    fun chooseAccount() {
        signInError = null
        signIn()
    }

    // WebDAV: the form, open when setting it up or entering the password again.
    var davForm by remember { mutableStateOf(false) }
    var davUrl by remember { mutableStateOf("") }
    var davUser by remember { mutableStateOf("") }
    var davPass by remember { mutableStateOf("") }
    var davError by remember { mutableStateOf<String?>(null) }
    fun openDavForm() {
        (state.provider as? app.memi.engine.SyncProvider.WebDav)?.let { davUrl = it.url; davUser = it.user }
        davPass = ""
        davError = null
        davForm = true
    }

    // A folder: the system picker, keeping its permission across restarts.
    val context = androidx.compose.ui.platform.LocalContext.current
    val pickFolder = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenDocumentTree()
    ) { uri ->
        if (uri == null) return@rememberLauncherForActivityResult
        val flags = android.content.Intent.FLAG_GRANT_READ_URI_PERMISSION or android.content.Intent.FLAG_GRANT_WRITE_URI_PERMISSION
        runCatching { context.contentResolver.takePersistableUriPermission(uri, flags) }
        signInError = null
        sync.launch { connectFolder(uri.toString())?.let { signInError = it } }
    }

    // Secure because it can show the recovery key.
    BentoSheet(onDismiss, secure = true) {
        SheetTitle("Sync")
        val problem = state.problem
        when {
            problem is SyncManager.Problem.NeedsRecoveryKey -> RecoveryKeyEntry(sync, problem.confirm)
            davForm -> {
                Body("Memi keeps its encrypted files in one folder on your WebDAV server. On Nextcloud, the address is under Files › Settings › WebDAV, with a folder name added (it's made if it isn't there), and an app password comes from Settings › Security.")
                BentoField("Folder address", davUrl, { davUrl = it; davError = null }, placeholder = "https://cloud.example.com/…/Memi",
                    keyboardType = androidx.compose.ui.text.input.KeyboardType.Uri, capitalization = KeyboardCapitalization.None)
                BentoField("User name", davUser, { davUser = it; davError = null }, capitalization = KeyboardCapitalization.None)
                BentoField("Password", davPass, { davPass = it; davError = null }, placeholder = "Password or app password",
                    capitalization = KeyboardCapitalization.None, password = true)
                davError?.let { Text(it, style = BentoType.caption, color = colors.danger) }
                val ready = davUrl.isNotBlank() && davUser.isNotBlank() && davPass.isNotEmpty() && !state.busy
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SecondaryButton("Back", Modifier.weight(1f)) { davForm = false; davError = null }
                    PrimaryButton(if (state.busy) "Connecting…" else "Connect", Modifier.weight(1f), enabled = ready) {
                        val (u, n, p) = Triple(davUrl, davUser, davPass)
                        sync.launch {
                            val error = connectWebdav(u, n, p)
                            if (error == null) { davForm = false; davPass = "" } else davError = error
                        }
                    }
                }
            }
            !state.enabled -> {
                Body("Keep a copy of everything somewhere you choose, so a new phone, tablet or computer can pick up where you left off.")
                Body("It's encrypted on this phone before it leaves, so wherever it's kept can't read it. Nobody else, including us, ever gets a copy.")
                PlaceChoice(androidx.compose.material.icons.Icons.Rounded.CloudSync, "Google Drive",
                    if (state.busy) "Setting up…" else "A hidden folder in your Google account, away from your Drive files", enabled = !state.busy) { chooseAccount() }
                PlaceChoice(androidx.compose.material.icons.Icons.Rounded.Dns, "WebDAV", "Nextcloud, ownCloud, a NAS or another WebDAV server", enabled = !state.busy) { openDavForm() }
                PlaceChoice(androidx.compose.material.icons.Icons.Rounded.Folder, "A folder",
                    "One that Syncthing, Dropbox or another app keeps the same on each device", enabled = !state.busy) { pickFolder.launch(null) }
            }
            else -> {
                Status(
                    title = state.account?.let { "Syncing with $it" } ?: "Sync is on",
                    detail = when {
                        state.busy -> "Syncing…"
                        state.lastSyncedAt != null -> "Up to date · synced ${sinceLabel(state.lastSyncedAt!!)}"
                        else -> "Waiting to sync"
                    },
                )
                if (problem is SyncManager.Problem.NeedsSignIn) {
                    when (state.provider) {
                        is app.memi.engine.SyncProvider.WebDav -> {
                            Body("Your WebDAV server turned down the saved password. Enter it again to keep syncing.")
                            PrimaryButton("Enter the password", Modifier.fillMaxWidth()) { openDavForm() }
                        }
                        is app.memi.engine.SyncProvider.Folder -> {
                            Body("Memi can't open the sync folder any more. Choose it again.")
                            PrimaryButton("Choose the folder", Modifier.fillMaxWidth()) { pickFolder.launch(null) }
                        }
                        else -> {
                            Body("Google needs you to sign in again before Memi can sync.")
                            PrimaryButton("Choose a Google account", Modifier.fillMaxWidth()) { chooseAccount() }
                        }
                    }
                }
                if (problem is SyncManager.Problem.Failed) {
                    Body("The last sync didn't work (${problem.message}). Memi will try again by itself.")
                    if (state.provider is app.memi.engine.SyncProvider.Folder) SecondaryButton("Choose the folder again", Modifier.fillMaxWidth()) { pickFolder.launch(null) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SecondaryButton("Sync now", Modifier.weight(1f)) { sync.syncNow() }
                    SecondaryButton("Turn off", Modifier.weight(1f), danger = true) { sync.launch { turnOff(forgetKey = false) } }
                }
                state.joins.forEach { JoinCard(it) }
                AddComputer(sync)
                RecoveryKeyReveal(sync, state.keyFollows, state.keySaved)
            }
        }
        signInError?.let { Text(it, style = BentoType.caption, color = colors.danger) }
        val uri = LocalUriHandler.current
        Text(
            "How sync handles your data",
            style = BentoType.label, color = colors.ink,
            modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { uri.openUri("https://yourmemi.app/privacy/") }.padding(vertical = 8.dp),
        )
    }
}

/** Copies the key, marked as sensitive so Android 13+ hides it from clipboard previews and keyboards. */
private fun copySecret(context: android.content.Context, text: String) {
    val clip = android.content.ClipData.newPlainText("Recovery key", text)
    clip.description.extras = android.os.PersistableBundle().apply {
        putBoolean(android.content.ClipDescription.EXTRA_IS_SENSITIVE, true)
    }
    context.getSystemService(android.content.ClipboardManager::class.java)?.setPrimaryClip(clip)
}

/** One of the places sync can go, when turning it on. */
@Composable
private fun PlaceChoice(icon: androidx.compose.ui.graphics.vector.ImageVector, title: String, detail: String, enabled: Boolean, onClick: () -> Unit) {
    val colors = Bento.colors
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.card)
            .clickable(enabled = enabled, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
            .padding(horizontal = 16.dp, vertical = 14.dp),
        verticalAlignment = androidx.compose.ui.Alignment.CenterVertically,
    ) {
        androidx.compose.material3.Icon(icon, contentDescription = null, tint = colors.ink, modifier = Modifier.size(24.dp))
        androidx.compose.foundation.layout.Spacer(Modifier.width(14.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = BentoType.cardTitle, color = colors.ink)
            Text(detail, style = BentoType.label, color = colors.muted)
        }
        androidx.compose.material3.Icon(androidx.compose.material.icons.Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = colors.muted)
    }
}

@Composable
private fun Body(text: String) = Text(text, style = BentoType.body, color = Bento.colors.ink.copy(alpha = 0.8f))

@Composable
private fun Status(title: String, detail: String) {
    val colors = Bento.colors
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.card).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(title, style = BentoType.cardTitle, color = colors.ink)
        Text(detail, style = BentoType.label, color = colors.muted)
    }
}

/** On Home: sync is on, but they haven't said they've kept the recovery key anywhere. */
@Composable
fun KeyNudgeCard(onOpenSync: () -> Unit) {
    val sync = LocalSync.current
    val colors = Bento.colors
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.card).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("Save your recovery key", style = BentoType.cardTitle, color = colors.ink)
        Text(
            "If you lose this phone and your other devices, it's the only way to open your synced copy. Keep it somewhere safe, like a password manager.",
            style = BentoType.label, color = colors.muted,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PrimaryButton("Show key", Modifier.weight(1f)) { sync.revealKeyOnOpen = true; onOpenSync() }
            SecondaryButton("Not now", Modifier.weight(1f)) { sync.snoozeKeyNudge() }
        }
    }
}

/** Optional: the key written out, for setting up a phone that wasn't restored from this one. */
@Composable
private fun RecoveryKeyReveal(sync: SyncManager, follows: Boolean, keySaved: Boolean) {
    val colors = Bento.colors
    val context = androidx.compose.ui.platform.LocalContext.current
    // Arriving from the reminder on Home: show it straight away.
    var shown by remember {
        mutableStateOf(if (sync.revealKeyOnOpen) sync.recoveryKey().also { sync.revealKeyOnOpen = false } else null)
    }
    Text(
        when {
            follows -> "Your key is backed up with your screen lock, so a new phone set up from this one picks sync up by itself."
            else -> "Set a screen lock so your key can follow you to a new phone, or keep the recovery key somewhere safe."
        },
        style = BentoType.caption, color = colors.muted,
    )
    val key = shown
    if (key == null) {
        SecondaryButton("Show recovery key", Modifier.fillMaxWidth()) { shown = sync.recoveryKey() ?: "No key on this phone" }
    } else {
        Text(key, style = BentoType.label.copy(fontFamily = FontFamily.Monospace), color = colors.ink,
            modifier = Modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(colors.track).padding(12.dp))
        SecondaryButton("Copy", Modifier.fillMaxWidth()) { copySecret(context, key) }
        if (!keySaved) PrimaryButton("I've saved it somewhere safe", Modifier.fillMaxWidth()) { sync.keySaved() }
    }
}

/** On the phone: scan the code a computer is showing, to give it the key. */
@Composable
private fun AddComputer(sync: SyncManager) {
    val colors = Bento.colors
    var message by remember { mutableStateOf<String?>(null) }
    val scan = rememberQrScanner(
        onScanned = { code -> message = "Sending…"; sync.launch { message = sendKeyTo(code) } },
        onError = { message = it },
    ) ?: return
    SecondaryButton("Add a computer", Modifier.fillMaxWidth()) { message = null; scan() }
    Text(
        message ?: "Open Sync in Memi on the computer, sign in, then scan the code it shows.",
        style = BentoType.caption, color = colors.muted,
    )
}

/**
 * Another device asking for the key. Shown in Sync and at the top of Home; the codes on both
 * devices should match before you allow it.
 */
@Composable
fun JoinCard(join: SyncManager.Join) {
    val sync = LocalSync.current
    val colors = Bento.colors
    var message by remember(join.fileId) { mutableStateOf<String?>(null) }
    Column(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.card).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("${join.device} wants to join your Memi", style = BentoType.cardTitle, color = colors.ink)
        Text("Allow it only if it shows this code:", style = BentoType.label, color = colors.muted)
        Text(join.confirm, style = BentoType.cardTitle.copy(fontFamily = FontFamily.Monospace), color = colors.ink)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            PrimaryButton("Allow", Modifier.weight(1f)) { message = "Sending…"; sync.launch { message = approveJoin(join.fileId) } }
            SecondaryButton("Not mine", Modifier.weight(1f)) { sync.launch { declineJoin(join.fileId) } }
        }
        message?.let { Text(it, style = BentoType.caption, color = colors.muted) }
    }
}

/**
 * When this phone has no key yet: it asks your other devices for it (they show an Allow card with
 * the same code), or you can type the recovery key.
 */
@Composable
private fun RecoveryKeyEntry(sync: SyncManager, confirm: String?) {
    val colors = Bento.colors
    var text by remember { mutableStateOf("") }
    var wrong by remember { mutableStateOf(false) }
    Body("Memi is already syncing on another device. To join it, this phone needs its key.")
    if (confirm != null) {
        Column(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.card).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text("On any device that's already syncing", style = BentoType.cardTitle, color = colors.ink)
            Text("Open Memi there and tap Allow when it asks about this phone. It shows this code:", style = BentoType.label, color = colors.muted)
            Text(confirm, style = BentoType.cardTitle.copy(fontFamily = FontFamily.Monospace), color = colors.ink)
        }
    }
    Body("Or type the recovery key from Settings → Sync → Show recovery key on that device.")
    BentoField("Recovery key", text, { text = it; wrong = false }, placeholder = "xxxxxxxx-xxxxxxxx-…", capitalization = KeyboardCapitalization.None,
        // A password-type keyboard doesn't learn or suggest what's typed. The text stays visible.
        keyboardType = androidx.compose.ui.text.input.KeyboardType.Password)
    if (wrong) Text("That key doesn't open this backup.", style = BentoType.caption, color = colors.danger)
    PrimaryButton("Unlock backup", Modifier.fillMaxWidth(), enabled = text.isNotBlank()) {
        sync.launch { if (!useRecoveryKey(text)) wrong = true }
    }
    SecondaryButton("Cancel", Modifier.fillMaxWidth()) { sync.launch { turnOff(forgetKey = false) } }

    // Nobody has the key any more: the only way forward is a fresh backup with a new key.
    var confirmStartOver by remember { mutableStateOf(false) }
    Text(
        "Lost your recovery key? Start over",
        style = BentoType.label, color = colors.danger,
        modifier = Modifier.clip(RoundedCornerShape(10.dp)).clickable { confirmStartOver = true }.padding(vertical = 8.dp),
    )
    if (confirmStartOver) {
        app.memi.ui.ConfirmDialog(
            title = "Start sync over?",
            body = "Without its recovery key, nobody can open the synced copy, so Memi will delete it and make a new one from what's on this phone. You'll get a new recovery key. Your other devices will ask for it before they sync again, and anything that's only on them stays there until you set them up.",
            confirm = "Start over",
            dismiss = "Cancel",
            onDismiss = { confirmStartOver = false },
            onConfirm = { confirmStartOver = false; sync.launch { startOver() } },
        )
    }
}

private fun sinceLabel(millis: Long): String {
    val mins = (System.currentTimeMillis() - millis) / 60_000
    return when {
        mins < 1 -> "just now"
        mins < 60 -> "$mins min ago"
        mins < 24 * 60 -> "${mins / 60} h ago"
        else -> "${mins / (24 * 60)} d ago"
    }
}
