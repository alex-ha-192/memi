@file:OptIn(ExperimentalFoundationApi::class)

package app.memi.ui.components

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.hoverable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsHoveredAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.MoreHoriz
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Outcome
import app.memi.data.Status
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion

/** Everything you can do to a note from its menu. */
class NoteActions(
    val collections: List<NoteCollection>,
    val onDone: (Note, Int) -> Unit,
    val onReopen: (Note) -> Unit,
    val onMove: (Note, NoteCollection) -> Unit,
    val onEdit: (Note) -> Unit,
    val onDelete: (Note) -> Unit,
)

/**
 * A note as a white card. Tap to edit; press and hold (or right-click, or the ⋯ that appears on
 * hover with a mouse) for Hit, Miss, Done, Move, Edit and Delete.
 */
@Composable
fun NoteCard(
    note: Note,
    title: String,
    subtitle: String,
    actions: NoteActions,
    modifier: Modifier = Modifier,
    leading: @Composable (() -> Unit)? = null,
    tag: Triple<String, Color, Color>? = null,
    background: Color = Bento.colors.card,
    onClick: (() -> Unit)? = null,
) {
    val colors = Bento.colors
    var menuOpen by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    val hovered by interaction.collectIsHoveredAsState()
    val hasMouse = LocalHasMouse.current

    Box(modifier) {
        Row(
            Modifier
                .fillMaxWidth()
                .pressScale(interaction, 0.98f)
                .clip(RoundedCornerShape(22.dp))
                .background(background)
                .hoverable(interaction)
                .pressable(onClick = onClick ?: { actions.onEdit(note) }, onMenu = { menuOpen = true }, interaction = interaction)
                .padding(horizontal = 14.dp, vertical = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            leading?.invoke()
            Column(Modifier.weight(1f)) {
                OneLine(title, BentoType.itemTitle, colors.ink)
                if (subtitle.isNotEmpty()) {
                    Spacer(Modifier.height(2.dp))
                    OneLine(subtitle, BentoType.caption, colors.muted)
                }
            }
            if (tag != null) TagChip(tag.first, tag.second, tag.third)
            AnimatedVisibility(
                visible = hasMouse && (hovered || menuOpen),
                enter = fadeIn(Motion.soft()) + scaleIn(Motion.settle(), initialScale = 0.8f),
                exit = fadeOut(Motion.soft()) + scaleOut(Motion.soft(), targetScale = 0.8f),
            ) {
                Box(
                    Modifier
                        .size(36.dp)
                        .clip(RoundedCornerShape(12.dp))
                        .background(colors.track)
                        .clickable(role = Role.Button, onClickLabel = "More actions") { menuOpen = true },
                    contentAlignment = Alignment.Center,
                ) {
                    Icon(Icons.Rounded.MoreHoriz, contentDescription = "More actions", tint = colors.ink, modifier = Modifier.size(20.dp))
                }
            }
        }
        NoteMenu(
            expanded = menuOpen,
            onDismiss = { menuOpen = false },
            note = note,
            actions = actions,
        )
    }
}

@Composable
fun NoteMenu(expanded: Boolean, onDismiss: () -> Unit, note: Note, actions: NoteActions) {
    val colors = Bento.colors
    var moving by remember(expanded) { mutableStateOf(false) }
    BentoMenu(expanded = expanded, onDismiss = onDismiss, offset = DpOffset(12.dp, 4.dp)) {
        run {
            Column(Modifier.widthIn(min = 248.dp).padding(horizontal = 8.dp, vertical = 2.dp)) {
                if (!moving) {
                    if (note.status == Status.OPEN) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(bottom = 6.dp)) {
                            BigChoice("Hit", Icons.Rounded.Check, colors.hit, colors.hitInk, Modifier.weight(1f)) {
                                onDismiss(); actions.onDone(note, Outcome.HIT)
                            }
                            BigChoice("Miss", Icons.Rounded.Close, colors.miss, colors.missInk, Modifier.weight(1f)) {
                                onDismiss(); actions.onDone(note, Outcome.MISS)
                            }
                        }
                        MenuRow("Mark done, decide later", Icons.Rounded.Check) { onDismiss(); actions.onDone(note, Outcome.NONE) }
                    } else {
                        if (note.outcome != Outcome.HIT) MenuRow("It was a hit", Icons.Rounded.Check) { onDismiss(); actions.onDone(note, Outcome.HIT) }
                        if (note.outcome != Outcome.MISS) MenuRow("It was a miss", Icons.Rounded.Close) { onDismiss(); actions.onDone(note, Outcome.MISS) }
                        MenuRow("Not done yet", Icons.Rounded.Replay) { onDismiss(); actions.onReopen(note) }
                    }
                    MenuRow("Move to…", Icons.AutoMirrored.Rounded.ArrowForward) { moving = true }
                    MenuRow("Edit", Icons.Rounded.Edit) { onDismiss(); actions.onEdit(note) }
                    MenuRow("Delete", Icons.Rounded.DeleteOutline, tint = colors.danger) { onDismiss(); actions.onDelete(note) }
                } else {
                    MenuRow("Back", Icons.AutoMirrored.Rounded.ArrowBack) { moving = false }
                    actions.collections.filter { it.personId == note.personId && it.id != note.collectionId }.forEach { c ->
                        MenuRow(c.name, null, swatch = colors.strips[c.slot % colors.strips.size]) {
                            onDismiss(); actions.onMove(note, c)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun BigChoice(label: String, icon: ImageVector, bg: Color, ink: Color, modifier: Modifier, onClick: () -> Unit) {
    Column(
        modifier
            .height(68.dp)
            .clip(RoundedCornerShape(18.dp))
            .background(bg)
            .clickable(role = Role.Button, onClick = onClick),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center,
    ) {
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(22.dp))
        Spacer(Modifier.height(2.dp))
        Text(label, style = BentoType.cardTitle, color = ink)
    }
}

@Composable
fun MenuRow(label: String, icon: ImageVector?, tint: Color = Bento.colors.ink, swatch: Color? = null, onClick: () -> Unit) {
    Row(
        Modifier
            .fillMaxWidth()
            .height(46.dp)
            .clip(RoundedCornerShape(14.dp))
            .clickable(role = Role.Button, onClick = onClick)
            .padding(horizontal = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (icon != null) Icon(icon, contentDescription = null, tint = tint, modifier = Modifier.size(20.dp))
        if (swatch != null) Box(Modifier.size(14.dp).clip(RoundedCornerShape(5.dp)).background(swatch))
        Spacer(Modifier.width(12.dp))
        Text(label, style = BentoType.itemTitle, color = tint)
    }
}

/** ✓ for a hit, ✕ for a miss, a soft dot for done without a verdict. */
@Composable
fun OutcomeMark(outcome: Int) {
    val colors = Bento.colors
    val (bg, ink, icon) = when (outcome) {
        Outcome.HIT -> Triple(colors.hit, colors.hitInk, Icons.Rounded.Check)
        Outcome.MISS -> Triple(colors.miss, colors.missInk, Icons.Rounded.Close)
        else -> Triple(colors.track, colors.muted, null)
    }
    Box(
        Modifier.size(24.dp).clip(RoundedCornerShape(8.dp)).background(bg),
        contentAlignment = Alignment.Center,
    ) {
        if (icon != null) Icon(icon, contentDescription = if (outcome == Outcome.HIT) "Hit" else "Miss", tint = ink, modifier = Modifier.size(15.dp))
        else Box(Modifier.size(6.dp).clip(RoundedCornerShape(3.dp)).background(ink))
    }
}
