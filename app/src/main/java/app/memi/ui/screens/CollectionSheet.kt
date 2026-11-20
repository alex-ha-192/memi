package app.memi.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import app.memi.data.Dates
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Person
import app.memi.data.Status
import app.memi.ui.components.NoteActions
import app.memi.ui.components.NoteCard
import app.memi.ui.sheets.BentoSheet
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType

/**
 * One category for one person: add something at the top, everything open underneath.
 * Opens with the keyboard ready when the category is empty.
 */
@Composable
fun CollectionSheet(
    person: Person,
    collection: NoteCollection,
    notes: List<Note>,
    actions: NoteActions,
    onAdd: (String) -> Unit,
    onDismiss: () -> Unit,
) {
    val colors = Bento.colors
    val slot = collection.slot % colors.tiles.size
    val items = notes.filter { it.personId == person.id && it.collectionId == collection.id && it.status == Status.OPEN }
        .sortedWith(compareBy({ it.sortOrder }, { -it.mentionedAt }))
    var text by remember { mutableStateOf("") }
    val focus = remember { FocusRequester() }
    LaunchedEffect(Unit) { if (items.isEmpty()) runCatching { focus.requestFocus() } }
    val add = {
        if (text.isNotBlank()) {
            onAdd(text.trim()); text = ""
        }
    }

    BentoSheet(onDismiss) {
        Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(top = 4.dp)) {
            Box(Modifier.width(14.dp).height(14.dp).clip(RoundedCornerShape(5.dp)).background(colors.strips[slot % colors.strips.size]))
            Spacer(Modifier.width(10.dp))
            Text("${collection.name} · ${person.firstName}", style = BentoType.tileTitle, color = colors.ink)
        }

        Row(
            Modifier.fillMaxWidth().clip(RoundedCornerShape(20.dp)).background(colors.tiles[slot]).padding(start = 16.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            BasicTextField(
                value = text,
                onValueChange = { text = it },
                textStyle = BentoType.input.copy(color = colors.ink),
                cursorBrush = SolidColor(colors.ink),
                maxLines = 4,
                keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences, imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { add() }),
                modifier = Modifier.weight(1f).padding(vertical = 12.dp).focusRequester(focus),
                decorationBox = { inner ->
                    Box {
                        if (text.isEmpty()) Text("Add to ${collection.name}…", style = BentoType.input, color = colors.tileInks[slot].copy(alpha = 0.75f))
                        inner()
                    }
                },
            )
            Spacer(Modifier.width(8.dp))
            val ready = text.isNotBlank()
            Box(
                Modifier.size(44.dp).clip(RoundedCornerShape(15.dp))
                    .background(if (ready) colors.ink else colors.card.copy(alpha = 0.6f))
                    .clickable(enabled = ready, role = Role.Button, onClickLabel = "Add") { add() },
                contentAlignment = Alignment.Center,
            ) {
                Icon(Icons.Rounded.Add, contentDescription = "Add", tint = if (ready) colors.bg else colors.muted, modifier = Modifier.size(22.dp))
            }
        }

        if (items.isEmpty()) {
            Text("Nothing here yet.", style = BentoType.body, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        } else {
            Text(if (app.memi.ui.components.LocalHasMouse.current) "Right-click an item, or use its ⋯ button, to mark it done, a hit or a miss." else "Press and hold an item to mark it done, a hit or a miss.", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        }
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            items.forEach { n ->
                val due = n.dueAt?.let { Dates.toDate(it) }
                NoteCard(
                    note = n,
                    title = n.text,
                    subtitle = "Added ${Dates.ago(n.mentionedAt)}",
                    actions = actions,
                    background = colors.card,
                    leading = { Box(Modifier.width(8.dp).height(32.dp).clip(RoundedCornerShape(4.dp)).background(colors.strips[slot % colors.strips.size])) },
                    tag = due?.let { Triple("Due ${Dates.shortDayLabel(it)}", colors.tiles[slot], colors.tileInks[slot]) },
                )
            }
        }
    }
}
