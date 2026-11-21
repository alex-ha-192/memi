package app.memi.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Devices
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.memi.platform.PlatformBackHandler
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType

/**
 * The first time Memi opens on a device with nothing in it: has this person used Memi before?
 * If so, they sign in to Google and another of their devices lets this one in. If not, they choose
 * between syncing with Google and keeping everything on this phone. Both answers carry the same
 * weight, nothing is chosen for them, and it's never asked again (Sync stays in Settings).
 *
 * [onDone] is called once, with whether to open Sync next (to pick Google Drive, WebDAV or a folder).
 */
@Composable
fun FirstLaunch(onDone: (openSync: Boolean) -> Unit) {
    val colors = Bento.colors
    var step by rememberSaveable { mutableStateOf(Step.Question) }
    var error by rememberSaveable { mutableStateOf<String?>(null) }
    fun chooseAccount() { error = null; onDone(true) }
    PlatformBackHandler(step == Step.Choice) { step = Step.Question; error = null }

    Box(
        Modifier
            .fillMaxSize()
            .background(colors.bg)
            .windowInsetsPadding(WindowInsets.statusBars)
            .windowInsetsPadding(WindowInsets.navigationBars),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            Modifier
                .widthIn(max = 480.dp)
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 32.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
                Box(Modifier.size(148.dp).clip(CircleShape).background(colors.tiles[3]), contentAlignment = Alignment.Center) {
                    Mascot(Modifier.size(104.dp))
                }
            }
            when (step) {
                Step.Question -> {
                    Heading("Have you used Memi before?", "If Memi is on another of your devices, this phone can pick up where it left off.")
                    Spacer(Modifier.height(4.dp))
                    Choice(
                        Icons.Rounded.Devices, colors.tiles[3], colors.tileInks[3],
                        "Yes, on another device",
                        "Connect to the same place it syncs to (Google Drive, WebDAV or a folder), then allow this phone from the other device.",
                    ) { chooseAccount() }
                    Choice(
                        Icons.Rounded.AutoAwesome, colors.gift, colors.giftInk,
                        "No, I'm new",
                        "Start fresh. Next, you choose whether to sync or keep everything on this phone.",
                    ) { error = null; step = Step.Choice }
                }
                Step.Choice -> {
                    Heading("Hi, I'm Memi", "I keep the little things about the people you care about: gift ideas, plans, and what they mentioned last time.")
                    Text(
                        "USE MEMI ON MORE THAN ONE DEVICE?",
                        style = BentoType.label, color = colors.muted, modifier = Modifier.padding(top = 4.dp),
                    )
                    Choice(
                        Icons.Rounded.CloudSync, colors.tiles[3], colors.tileInks[3],
                        "Turn on sync",
                        "Your phone, computer and browser stay in step, through Google Drive, WebDAV or a folder. It's encrypted on this phone before it leaves.",
                    ) { chooseAccount() }
                    Choice(
                        Icons.Rounded.PhoneAndroid, colors.plan, colors.planInk,
                        "Just this phone",
                        "Everything stays here. You can turn on sync later in Settings.",
                    ) { onDone(false) }
                }
            }
            error?.let { Text(it, style = BentoType.body, color = colors.danger) }
            Spacer(Modifier.weight(1f))
            val uri = LocalUriHandler.current
            Text(
                "Memi has no servers and no accounts, and nobody else gets a copy, including us.",
                style = BentoType.caption, color = colors.muted, textAlign = TextAlign.Center, modifier = Modifier.fillMaxWidth(),
            )
            Text(
                "How sync handles your data",
                style = BentoType.label, color = colors.tileInks[3], textAlign = TextAlign.Center,
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .clickable(role = Role.Button) { uri.openUri("https://yourmemi.app/privacy/") }
                    .padding(vertical = 12.dp),
            )
        }
    }
}

private enum class Step { Question, Choice }

@Composable
private fun Heading(title: String, body: String) {
    val colors = Bento.colors
    Column(Modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
        Text(title, style = BentoType.display, color = colors.ink, textAlign = TextAlign.Center)
        Text(body, style = BentoType.body, color = colors.muted, textAlign = TextAlign.Center)
    }
}

/** One of two answers, drawn exactly alike so neither is pushed. */
@Composable
private fun Choice(icon: ImageVector, chip: Color, ink: Color, title: String, body: String, onClick: () -> Unit) {
    val colors = Bento.colors
    Row(
        Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(22.dp))
            .background(colors.card)
            .border(1.5.dp, colors.line, RoundedCornerShape(22.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(16.dp),
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(Modifier.size(44.dp).clip(RoundedCornerShape(14.dp)).background(chip), contentAlignment = Alignment.Center) {
            Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(22.dp))
        }
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(title, style = BentoType.cardTitle.copy(fontSize = BentoType.section.fontSize), color = colors.ink)
            Text(body, style = BentoType.body, color = colors.muted)
        }
    }
}
