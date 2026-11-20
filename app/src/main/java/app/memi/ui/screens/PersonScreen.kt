package app.memi.ui.screens

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
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
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.memi.data.Dates
import app.memi.data.Occasion
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Person
import app.memi.data.Status
import app.memi.ui.components.BentoMenu
import app.memi.ui.components.LocalHasMouse
import app.memi.ui.components.MenuRow
import app.memi.ui.components.NoteActions
import app.memi.ui.components.NoteMenu
import app.memi.ui.components.OneLine
import app.memi.ui.components.OutcomeMark
import app.memi.ui.components.PersonPhoto
import app.memi.ui.components.SectionHeader
import app.memi.ui.components.TagChip
import app.memi.ui.components.Tile
import app.memi.ui.components.pressable
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion

class PersonCallbacks(
    val onEditPerson: () -> Unit,
    val onRemovePerson: () -> Unit,
    val onOpenCollection: (NoteCollection) -> Unit,
    val onCollectionMenu: (NoteCollection, CollectionAction) -> Unit,
    val onNewCollection: () -> Unit,
)

enum class CollectionAction { EDIT, MOVE_EARLIER, MOVE_LATER, DELETE }

/**
 * One person. On a phone it follows Home's rhythm: name, a peach tile with their photo,
 * their categories as tiles (tap one to add to it), then what's done (✓ hit, ✕ miss).
 * In a wider pane the photo becomes a portrait card beside the tiles.
 */
@Composable
fun PersonScreen(
    person: Person,
    notes: List<Note>,
    collections: List<NoteCollection>,
    actions: NoteActions,
    callbacks: PersonCallbacks,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
) {
    val mine = remember(notes, person.id) { notes.filter { it.personId == person.id } }
    val categories = remember(collections, person.id) { collections.filter { it.personId == person.id }.sortedBy { it.sortOrder } }
    val open = remember(mine) { mine.filter { it.status == Status.OPEN } }
    val done = remember(mine) { mine.filter { it.status == Status.DONE }.sortedByDescending { it.doneAt ?: it.mentionedAt } }

    BoxWithConstraints(modifier.fillMaxSize()) {
        if (maxWidth >= 600.dp) WidePerson(person, categories, open, done, actions, callbacks, contentPadding)
        else PhonePerson(person, categories, open, done, actions, callbacks, contentPadding)
    }
}

@Composable
private fun PhonePerson(
    person: Person,
    collections: List<NoteCollection>,
    open: List<Note>,
    done: List<Note>,
    actions: NoteActions,
    callbacks: PersonCallbacks,
    contentPadding: PaddingValues,
) {
    val colors = Bento.colors
    var personMenu by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "header") {
            Box {
                Column(
                    Modifier.fillMaxWidth().pressable(null, { personMenu = true }, interaction).padding(start = 22.dp, end = 22.dp, top = 28.dp)
                ) {
                    Text(person.relationship.ifBlank { "Press and hold to edit" }, style = BentoType.label, color = colors.muted)
                    Spacer(Modifier.height(4.dp))
                    Text(person.name, style = BentoType.display, color = colors.ink, maxLines = 2, overflow = TextOverflow.Ellipsis)
                }
                PersonMenu(personMenu, { personMenu = false }, callbacks)
            }
        }
        item(key = "hero") {
            HeroTile(person, open, collections, onMenu = { personMenu = true }, Modifier.padding(start = 22.dp, end = 22.dp, top = 22.dp))
        }
        item(key = "catHeader") {
            SectionHeader("Categories", Modifier.padding(start = 22.dp, end = 22.dp, top = 28.dp, bottom = 12.dp), trailing = "tap to add")
        }
        item(key = "tiles") {
            CollectionTiles(person, collections, open, callbacks, columns = 2, extraTile = null, Modifier.padding(horizontal = 22.dp))
        }
        if (done.isNotEmpty()) {
            item(key = "doneHeader") {
                SectionHeader("Done", Modifier.padding(start = 22.dp, end = 22.dp, top = 28.dp, bottom = 10.dp), count = done.size)
            }
            item(key = "done") {
                DoneList(done, actions, columns = 1, Modifier.padding(horizontal = 22.dp))
            }
        }
    }
}

@Composable
private fun WidePerson(
    person: Person,
    collections: List<NoteCollection>,
    open: List<Note>,
    done: List<Note>,
    actions: NoteActions,
    callbacks: PersonCallbacks,
    contentPadding: PaddingValues,
) {
    val colors = Bento.colors
    LazyColumn(Modifier.fillMaxSize(), contentPadding = contentPadding) {
        item(key = "top") {
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                PhotoCard(person, callbacks, Modifier.width(248.dp).fillMaxHeight().heightIn(min = 310.dp))
                CollectionTiles(
                    person, collections, open, callbacks, columns = 2,
                    extraTile = { m -> DatesTile(person, collections, callbacks, m) },
                    modifier = Modifier.weight(1f),
                )
            }
        }
        if (done.isNotEmpty()) {
            item(key = "done") {
                Column(
                    Modifier.padding(top = 12.dp).fillMaxWidth().clip(RoundedCornerShape(24.dp)).background(colors.card).padding(start = 22.dp, end = 22.dp, top = 16.dp, bottom = 8.dp)
                ) {
                    SectionHeader("Done", Modifier.padding(bottom = 4.dp), count = done.size)
                    DoneList(done, actions, columns = 3, Modifier, inCard = true)
                }
            }
        }
    }
}

@Composable
private fun PhotoCard(person: Person, callbacks: PersonCallbacks, modifier: Modifier) {
    val colors = Bento.colors
    val hasMouse = LocalHasMouse.current
    var menu by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    Box(modifier.clip(RoundedCornerShape(30.dp)).pressable(null, { if (hasMouse) callbacks.onEditPerson() else { menu = true } }, interaction)) {
        PersonPhoto(person, Modifier.fillMaxSize(), RoundedCornerShape(30.dp))
        // With a mouse, a pencil edits the person; on touch, press and hold opens the menu instead.
        AnimatedVisibility(
            visible = hasMouse,
            enter = fadeIn(Motion.soft()),
            exit = fadeOut(Motion.soft()),
            modifier = Modifier.align(Alignment.TopStart).padding(12.dp),
        ) {
            Box(
                Modifier.size(40.dp).clip(RoundedCornerShape(14.dp)).background(colors.sheet.copy(alpha = 0.92f))
                    .clickable(role = Role.Button, onClickLabel = "Edit ${person.firstName}") { callbacks.onEditPerson() },
                contentAlignment = Alignment.Center,
            ) { Icon(Icons.Rounded.Edit, contentDescription = "Edit ${person.firstName}", tint = colors.ink, modifier = Modifier.size(18.dp)) }
        }
        Column(
            Modifier.align(Alignment.BottomCenter).padding(10.dp).fillMaxWidth().clip(RoundedCornerShape(18.dp)).background(colors.sheet).padding(horizontal = 14.dp, vertical = 12.dp)
        ) {
            OneLine(person.name, BentoType.section.copy(letterSpacing = BentoType.tileTitle.letterSpacing), colors.ink)
            if (person.relationship.isNotBlank()) OneLine(person.relationship, BentoType.caption, colors.muted)
        }
        PersonMenu(menu, { menu = false }, callbacks)
    }
}

@Composable
private fun PersonMenu(expanded: Boolean, onDismiss: () -> Unit, callbacks: PersonCallbacks) {
    val colors = Bento.colors
    BentoMenu(expanded, onDismiss) {
        Column(Modifier.padding(horizontal = 8.dp)) {
            MenuRow("Edit", Icons.Rounded.Edit) { onDismiss(); callbacks.onEditPerson() }
            MenuRow("Remove", Icons.Rounded.DeleteOutline, tint = colors.danger) { onDismiss(); callbacks.onRemovePerson() }
        }
    }
}

@Composable
private fun HeroTile(person: Person, open: List<Note>, collections: List<NoteCollection>, onMenu: () -> Unit, modifier: Modifier) {
    val colors = Bento.colors
    val next = Dates.nextBirthday(person)
    val birthdayIds = collections.filter { it.occasion == Occasion.BIRTHDAY }.map { it.id }.toSet()
    val ideas = open.count { it.collectionId in birthdayIds }
    Tile(
        modifier.fillMaxWidth().height(136.dp),
        color = colors.gift,
        shape = RoundedCornerShape(28.dp),
        onMenu = onMenu,
        contentPadding = 0.dp,
    ) {
        Row(Modifier.fillMaxSize()) {
            Column(Modifier.weight(1f).fillMaxHeight().padding(horizontal = 20.dp, vertical = 18.dp), verticalArrangement = Arrangement.SpaceBetween) {
                if (next != null) {
                    TagChip("${Dates.dayLabel(next)} · ${Dates.inDays(next)}", colors.card, colors.giftInk)
                    Column {
                        val turning = Dates.turningAge(person)
                        Text(if (turning != null) "Turning $turning" else "Birthday", style = BentoType.tileTitle, color = colors.ink)
                        Spacer(Modifier.height(5.dp))
                        Text(
                            "Birthday · ${Dates.shortDate(next)}" + if (ideas > 0) " · $ideas ${if (ideas == 1) "idea" else "ideas"} for it" else "",
                            style = BentoType.label, color = colors.giftInk, maxLines = 1, overflow = TextOverflow.Ellipsis,
                        )
                    }
                } else {
                    TagChip(if (open.size == 1) "1 open" else "${open.size} open", colors.card, colors.giftInk)
                    Column {
                        Text(person.firstName, style = BentoType.tileTitle, color = colors.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Spacer(Modifier.height(5.dp))
                        Text("Hold to add a birthday", style = BentoType.label, color = colors.giftInk)
                    }
                }
            }
            PersonPhoto(person, Modifier.width(136.dp).fillMaxHeight(), RoundedCornerShape(0.dp))
        }
    }
}

/** Every category as a tile, plus a dashed "New category" tile. Tap to add, hold to rename or reorder. */
@Composable
private fun CollectionTiles(
    person: Person,
    collections: List<NoteCollection>,
    open: List<Note>,
    callbacks: PersonCallbacks,
    columns: Int,
    extraTile: (@Composable (Modifier) -> Unit)?,
    modifier: Modifier,
) {
    val cells = mutableListOf<@Composable (Modifier) -> Unit>()
    if (extraTile != null) cells.add(extraTile)
    collections.forEachIndexed { index, c ->
        val items = open.filter { it.collectionId == c.id }.sortedByDescending { it.mentionedAt }
        val first = index == 0
        val last = index == collections.lastIndex
        cells.add { m -> CollectionTile(person, c, items, first, last, callbacks, m) }
    }
    cells.add { m -> NewCollectionTile(callbacks.onNewCollection, m) }
    Column(modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        cells.chunked(columns).forEach { row ->
            Row(Modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                row.forEach { cell -> cell(Modifier.weight(1f).fillMaxHeight()) }
                repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
            }
        }
    }
}

@Composable
private fun CollectionTile(
    person: Person,
    c: NoteCollection,
    items: List<Note>,
    first: Boolean,
    last: Boolean,
    callbacks: PersonCallbacks,
    modifier: Modifier,
) {
    val colors = Bento.colors
    val slot = c.slot % colors.tiles.size
    var menu by remember { mutableStateOf(false) }
    Box(modifier) {
        Tile(
            Modifier.fillMaxSize().heightIn(min = 112.dp),
            color = colors.tiles[slot],
            onClick = { callbacks.onOpenCollection(c) },
            onMenu = { menu = true },
        ) {
            Column(Modifier.fillMaxWidth()) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(c.name, style = BentoType.cardTitle, color = colors.ink, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = Modifier.weight(1f, fill = false))
                    Text("  ${items.size}", style = BentoType.cardTitle, color = colors.tileInks[slot])
                }
                occasionLabel(c, person)?.let {
                    Text(it, style = BentoType.caption, color = colors.tileInks[slot])
                }
                Spacer(Modifier.height(10.dp))
                if (items.isEmpty()) {
                    Text(if (LocalHasMouse.current) "+ Click to add" else "+ Tap to add", style = BentoType.label.copy(fontWeight = FontWeight.Bold), color = colors.tileInks[slot])
                } else {
                    items.take(3).forEachIndexed { i, n ->
                        OneLine(
                            n.text,
                            if (i == 0) BentoType.label.copy(fontWeight = FontWeight.SemiBold) else BentoType.label.copy(fontWeight = FontWeight.Medium),
                            if (i == 0) colors.ink else colors.ink.copy(alpha = 0.72f),
                        )
                    }
                }
            }
        }
        BentoMenu(expanded = menu, onDismiss = { menu = false }) {
            Column(Modifier.padding(horizontal = 8.dp)) {
                MenuRow("Edit", Icons.Rounded.Edit) { menu = false; callbacks.onCollectionMenu(c, CollectionAction.EDIT) }
                if (!first) MenuRow("Move earlier", Icons.AutoMirrored.Rounded.ArrowBack) { menu = false; callbacks.onCollectionMenu(c, CollectionAction.MOVE_EARLIER) }
                if (!last) MenuRow("Move later", Icons.AutoMirrored.Rounded.ArrowForward) { menu = false; callbacks.onCollectionMenu(c, CollectionAction.MOVE_LATER) }
                MenuRow("Remove category", Icons.Rounded.DeleteOutline, tint = colors.danger) { menu = false; callbacks.onCollectionMenu(c, CollectionAction.DELETE) }
            }
        }
    }
}

@Composable
private fun NewCollectionTile(onClick: () -> Unit, modifier: Modifier) {
    val colors = Bento.colors
    Box(
        modifier
            .heightIn(min = 112.dp)
            .clip(RoundedCornerShape(24.dp))
            .border(2.dp, colors.handle, RoundedCornerShape(24.dp))
            .clickable(role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Text("+ New category", style = BentoType.itemTitle, color = colors.muted)
    }
}

@Composable
private fun DatesTile(person: Person, categories: List<NoteCollection>, callbacks: PersonCallbacks, modifier: Modifier) {
    val colors = Bento.colors
    Tile(modifier.heightIn(min = 112.dp), color = colors.card, onClick = callbacks.onEditPerson) {
        Column {
            Text("DATES", style = BentoType.tag.copy(letterSpacing = BentoType.tag.fontSize.times(0.05f)), color = colors.muted)
            Spacer(Modifier.height(10.dp))
            val next = Dates.nextBirthday(person)
            if (next != null) {
                Text("Birthday · ${Dates.shortDate(next)}", style = BentoType.body.copy(fontWeight = FontWeight.Bold), color = colors.ink)
            } else {
                Text("+ Add a birthday", style = BentoType.body.copy(fontWeight = FontWeight.Bold), color = colors.tileInks[3])
            }
            categories.filter { it.occasion == Occasion.DATE }.forEach { c ->
                Dates.nextOccasion(c, person)?.let { d ->
                    OneLine("${c.name} · ${Dates.shortDate(d)}", BentoType.label, colors.ink.copy(alpha = 0.8f))
                }
            }
        }
    }
}

@Composable
private fun DoneList(done: List<Note>, actions: NoteActions, columns: Int, modifier: Modifier, inCard: Boolean = false) {
    val colors = Bento.colors
    val shown = done.take(if (columns == 1) 12 else 18)
    val body = @Composable {
        Column {
            shown.chunked(columns).forEach { row ->
                Row(horizontalArrangement = Arrangement.spacedBy(28.dp)) {
                    row.forEach { n -> DoneRow(n, actions, Modifier.weight(1f)) }
                    repeat(columns - row.size) { Spacer(Modifier.weight(1f)) }
                }
            }
        }
    }
    if (inCard) Box(modifier) { body() }
    else Box(modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp)).background(colors.card).padding(horizontal = 16.dp, vertical = 4.dp)) { body() }
}

@Composable
private fun DoneRow(n: Note, actions: NoteActions, modifier: Modifier) {
    val colors = Bento.colors
    var menu by remember { mutableStateOf(false) }
    val interaction = remember { MutableInteractionSource() }
    Box(modifier) {
        Row(
            Modifier.fillMaxWidth().height(44.dp).pressable({ menu = true }, { menu = true }, interaction),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            OutcomeMark(n.outcome)
            Spacer(Modifier.width(12.dp))
            OneLine(n.text, BentoType.body, colors.ink, Modifier.weight(1f))
            Spacer(Modifier.width(8.dp))
            Text(Dates.monthLabel(n.doneAt ?: n.mentionedAt), style = BentoType.caption, color = colors.muted)
        }
        NoteMenu(menu, { menu = false }, n, actions)
    }
}

/** "for Sunday" when it's close, otherwise the date, for a category building up to something. */
private fun occasionLabel(c: NoteCollection, person: Person): String? {
    val next = Dates.nextOccasion(c, person) ?: return null
    val days = Dates.daysUntil(next)
    if (days > 60) return "for ${Dates.shortDate(next)}"
    val day = Dates.dayLabel(next)
    return "for ${if (day == "Today" || day == "Tomorrow") day.lowercase() else day}"
}
