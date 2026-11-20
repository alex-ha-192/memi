package app.memi.ui.sheets

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
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

/** Sync settings: pick a Google account once, and Memi keeps an encrypted copy there from then on. */
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

    // Secure because it can show the recovery key.
    BentoSheet(onDismiss, secure = true) {
        SheetTitle("Sync")
        val problem = state.problem
        when {
            problem is SyncManager.Problem.NeedsRecoveryKey -> RecoveryKeyEntry(sync, problem.confirm)
            !state.enabled -> {
                Body("Keep a copy of everything in your own Google account, so a new phone, tablet or computer can pick up where you left off.")
                Body("It's encrypted on this phone before it leaves, so Google can't read it, and it sits in a hidden folder only Your Memi can open, away from your Drive files. Nobody else, including us, ever gets a copy.")
                PrimaryButton(if (state.busy) "Setting up…" else "Choose a Google account", Modifier.fillMaxWidth(), enabled = !state.busy) { chooseAccount() }
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
                    Body("Google needs you to sign in again before Your Memi can sync.")
                    PrimaryButton("Choose a Google account", Modifier.fillMaxWidth()) { chooseAccount() }
                }
                if (problem is SyncManager.Problem.Failed) Body("The last sync didn't work (${problem.message}). Your Memi will try again by itself.")
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    SecondaryButton("Sync now", Modifier.weight(1f)) { sync.syncNow() }
                    SecondaryButton("Turn off", Modifier.weight(1f), danger = true) { sync.launch { turnOff(forgetKey = false) } }
                }
                state.joins.forEach { JoinCard(it) }
                AddComputer(sync)
                RecoveryKeyReveal(sync, state.keyFollows)
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

/** Optional: the key written out, for setting up a phone that wasn't restored from this one. */
@Composable
private fun RecoveryKeyReveal(sync: SyncManager, follows: Boolean) {
    val colors = Bento.colors
    val context = androidx.compose.ui.platform.LocalContext.current
    var shown by remember { mutableStateOf<String?>(null) }
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
    Body("Your Memi is already syncing on another device. To join it, this phone needs its key.")
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
    Body("Or type the recovery key from Theme → Sync → Show recovery key on that device.")
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
            body = "Without its recovery key, nobody can open the copy in your Google account, so Memi will delete it and make a new one from what's on this phone. You'll get a new recovery key. Your other devices will ask for it before they sync again, and anything that's only on them stays there until you set them up.",
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
