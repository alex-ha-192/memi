package app.memi.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DarkMode
import androidx.compose.material.icons.rounded.LightMode
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.dp
import app.memi.data.Appearance
import app.memi.data.ThemeChoice
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoColors
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion
import app.memi.ui.theme.Schemes
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.foundation.clickable

/**
 * The theme picker. It slides up from the bottom without dimming anything, so the header
 * buttons stay usable; the palette button toggles it. Every colour eases into place.
 */
@Composable
fun ThemePanel(
    choice: ThemeChoice,
    onScheme: (String) -> Unit,
    onAppearance: (Appearance) -> Unit,
    syncDetail: String,
    onSync: () -> Unit,
    reminders: Set<Int>,
    onReminders: (Set<Int>) -> Unit,
    modifier: Modifier = Modifier,
    onSaveBackup: () -> Unit = {},
    onOpenBackup: () -> Unit = {},
) {
    val colors = Bento.colors
    val modeLabel = when (choice.appearance) {
        Appearance.LIGHT -> "light"
        Appearance.DARK -> "dark"
        Appearance.SYSTEM -> "following your phone (${if (colors.isDark) "dark" else "light"})"
    }
    Column(
        modifier
            .fillMaxWidth()
            .shadow(24.dp, RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp), ambientColor = Color.Black.copy(alpha = 0.2f), spotColor = Color.Black.copy(alpha = 0.2f))
            .clip(RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp))
            .background(colors.sheet)
            .padding(start = 20.dp, end = 20.dp, top = 10.dp, bottom = 22.dp),
    ) {
        Box(Modifier.align(Alignment.CenterHorizontally).width(40.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(colors.handle))
        Spacer(Modifier.height(12.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text("Theme", style = BentoType.tileTitle.copy(fontSize = BentoType.section.fontSize), color = colors.ink, modifier = Modifier.weight(1f))
            Text("${choice.scheme} · $modeLabel", style = BentoType.label, color = colors.muted)
        }
        Spacer(Modifier.height(14.dp))
        Row(Modifier.selectableGroup(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Schemes.all.forEach { scheme ->
                val preview = if (colors.isDark) scheme.dark else scheme.light
                SchemePreview(
                    name = scheme.name,
                    preview = preview,
                    selected = scheme.name == choice.scheme,
                    onClick = { onScheme(scheme.name) },
                    modifier = Modifier.weight(1f),
                )
            }
        }
        Spacer(Modifier.height(16.dp))
        Row(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(18.dp))
                .background(colors.track)
                .padding(4.dp)
                .selectableGroup(),
            horizontalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            ModeOption("Light", Icons.Rounded.LightMode, choice.appearance == Appearance.LIGHT, Modifier.weight(1f)) { onAppearance(Appearance.LIGHT) }
            ModeOption("Dark", Icons.Rounded.DarkMode, choice.appearance == Appearance.DARK, Modifier.weight(1f)) { onAppearance(Appearance.DARK) }
            ModeOption("System", Icons.Rounded.PhoneAndroid, choice.appearance == Appearance.SYSTEM, Modifier.weight(1f)) { onAppearance(Appearance.SYSTEM) }
        }
        Spacer(Modifier.height(16.dp))
        BirthdayReminderChoice(reminders, onReminders)
        Spacer(Modifier.height(10.dp))
        Row(
            Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(18.dp))
                .background(colors.card)
                .clickable(role = Role.Button, onClick = onSync)
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(Icons.Rounded.CloudSync, contentDescription = null, tint = colors.ink, modifier = Modifier.size(22.dp))
            Spacer(Modifier.width(12.dp))
            Text("Sync", style = BentoType.cardTitle, color = colors.ink)
            Spacer(Modifier.width(10.dp))
            Text(syncDetail, style = BentoType.label, color = colors.muted, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f))
            Icon(Icons.AutoMirrored.Rounded.KeyboardArrowRight, contentDescription = null, tint = colors.muted)
        }
        Spacer(Modifier.height(16.dp))
        Row(verticalAlignment = Alignment.Bottom) {
            Text("Backup file", style = BentoType.cardTitle, color = colors.ink, modifier = Modifier.weight(1f))
            Text("Not encrypted", style = BentoType.label, color = colors.muted)
        }
        Spacer(Modifier.height(8.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            app.memi.ui.sheets.SecondaryButton("Save a backup", Modifier.weight(1f), onClick = onSaveBackup)
            app.memi.ui.sheets.SecondaryButton("Open a backup", Modifier.weight(1f), onClick = onOpenBackup)
        }
    }
}

/**
 * Birthday reminders: any of on the day, the day before and a week before; none is off. Turning
 * one on asks for permission to show notifications (Android 13+) the first time.
 */
@Composable
private fun BirthdayReminderChoice(days: Set<Int>, onChange: (Set<Int>) -> Unit) {
    val colors = Bento.colors
    val context = androidx.compose.ui.platform.LocalContext.current
    var pending by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf<Set<Int>?>(null) }
    var denied by androidx.compose.runtime.remember { androidx.compose.runtime.mutableStateOf(false) }
    val ask = androidx.activity.compose.rememberLauncherForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.RequestPermission()
    ) { granted ->
        denied = !granted
        pending?.let { if (granted) onChange(it) }
        pending = null
    }
    fun toggle(d: Int) {
        val next = if (d in days) days - d else days + d
        if (next.isNotEmpty() && !app.memi.platform.BirthdayReminders.canNotify(context) && android.os.Build.VERSION.SDK_INT >= 33) {
            pending = next
            ask.launch(android.Manifest.permission.POST_NOTIFICATIONS)
        } else onChange(next)
    }
    Row(verticalAlignment = Alignment.Bottom) {
        Text("Birthday reminders", style = BentoType.cardTitle, color = colors.ink, modifier = Modifier.weight(1f))
        Text(
            when {
                denied -> "Notifications are off for Memi"
                days.isEmpty() -> "Off"
                else -> "Around 9 am"
            },
            style = BentoType.label, color = if (denied) colors.danger else colors.muted,
        )
    }
    Spacer(Modifier.height(8.dp))
    Row(
        Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.track).padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        listOf(0 to "On the day", 1 to "Day before", 7 to "Week before").forEach { (d, text) ->
            val on = d in days
            val pill = if (colors.isDark) Color.White.copy(alpha = 0.12f) else colors.sheet
            val bg by animateColorAsState(if (on) pill else Color.Transparent, Motion.colour(), label = "remindBg")
            Box(
                Modifier.weight(1f).height(40.dp).clip(RoundedCornerShape(14.dp)).background(bg)
                    .clickable(role = Role.Checkbox) { toggle(d) },
                contentAlignment = Alignment.Center,
            ) {
                Text(text, style = BentoType.label, color = if (on) colors.ink else colors.muted, maxLines = 1)
            }
        }
    }
}

@Composable
private fun SchemePreview(name: String, preview: BentoColors, selected: Boolean, onClick: () -> Unit, modifier: Modifier) {
    val colors = Bento.colors
    // Each preview's colours ease too, so flipping light/dark morphs the swatches.
    val bg by animateColorAsState(preview.bg, Motion.colour(), label = "pBg")
    val gift by animateColorAsState(preview.gift, Motion.colour(), label = "pGift")
    val plan by animateColorAsState(preview.plan, Motion.colour(), label = "pPlan")
    val note by animateColorAsState(preview.note, Motion.colour(), label = "pNote")
    val dock by animateColorAsState(preview.dock, Motion.colour(), label = "pDock")
    val ring by animateDpAsState(if (selected) 2.dp else 0.dp, Motion.settle(), label = "ring")
    Column(
        modifier.selectable(selected = selected, role = Role.RadioButton, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .height(84.dp)
                .border(ring, colors.ink, RoundedCornerShape(18.dp))
                .padding(if (selected) 4.dp else 0.dp)
                .clip(RoundedCornerShape(if (selected) 14.dp else 18.dp))
                .background(bg)
                .padding(6.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Box(Modifier.fillMaxWidth().weight(1.4f).clip(RoundedCornerShape(10.dp)).background(gift))
            Row(Modifier.fillMaxWidth().weight(1f), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                Box(Modifier.weight(1f).fillMaxHeight().clip(RoundedCornerShape(7.dp)).background(plan))
                Box(Modifier.weight(1f).fillMaxHeight().clip(RoundedCornerShape(7.dp)).background(note))
            }
            Box(Modifier.fillMaxWidth().weight(0.6f).clip(RoundedCornerShape(6.dp)).background(dock))
        }
        Spacer(Modifier.height(6.dp))
        // The swatches are narrow, so with very large system text this label grows a little less
        // (up to 1.1×) instead of being cut off.
        val fontScale = androidx.compose.ui.platform.LocalDensity.current.fontScale
        val caption = BentoType.caption
        Text(
            name,
            style = caption.copy(
                fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
                fontSize = caption.fontSize * minOf(1f, 1.1f / fontScale),
            ),
            color = colors.ink, maxLines = 1, softWrap = false,
        )
    }
}

@Composable
private fun ModeOption(label: String, icon: androidx.compose.ui.graphics.vector.ImageVector, selected: Boolean, modifier: Modifier, onClick: () -> Unit) {
    val colors = Bento.colors
    val pill = if (colors.isDark) Color.White.copy(alpha = 0.12f) else colors.sheet
    val bg by animateColorAsState(if (selected) pill else Color.Transparent, Motion.colour(), label = "modeBg")
    val fg by animateColorAsState(if (selected) colors.ink else colors.muted, Motion.colour(), label = "modeFg")
    Row(
        modifier
            .height(44.dp)
            .clip(RoundedCornerShape(14.dp))
            .background(bg)
            .selectable(selected = selected, role = Role.RadioButton, onClick = onClick),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(icon, contentDescription = null, tint = fg, modifier = Modifier.size(17.dp))
        Spacer(Modifier.width(8.dp))
        Text(label, style = BentoType.itemTitle.copy(fontSize = BentoType.body.fontSize), color = fg)
    }
}
