package app.memi.ui.screens

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Image
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.PushPin
import androidx.compose.material3.Icon
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.Color
import app.memi.ui.components.BentoMenu
import app.memi.ui.components.FitText
import app.memi.ui.components.MenuRow
import app.memi.ui.components.pressScale
import androidx.compose.animation.animateContentSize
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.CloudDone
import androidx.compose.material.icons.rounded.CloudOff
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Sync
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.memi.data.Dates
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Person
import app.memi.ui.Insights
import app.memi.ui.components.HeaderButton
import app.memi.ui.components.NoteActions
import app.memi.ui.components.NoteCard
import app.memi.ui.components.OneLine
import app.memi.ui.components.PersonPhoto
import app.memi.ui.components.SectionHeader
import app.memi.ui.components.TagChip
import app.memi.ui.components.Tile
import app.memi.ui.components.pressable
import app.memi.ui.components.ReorderState
import app.memi.ui.components.rememberReorderState
import app.memi.ui.components.reorderable
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion

/** What you can do to a person from their long-press menu on Home. */
class PeopleActions(
    val onPin: (Person, Boolean) -> Unit,
    val onMove: (Person, Int) -> Unit,
    val onEdit: (Person) -> Unit,
    val onRemove: (Person) -> Unit,
    /** Drag and drop: put the first person in the second one's place. */
    val onPlace: (Long, Long) -> Unit = { _, _ -> },
)

data class HomeModel(
    val people: List<Person>,
    val notes: List<Note>,
    val collections: List<NoteCollection>,
    val freshCollapsed: Boolean = false,
    /** Other devices asking for the sync key. */
    val joins: List<app.memi.sync.SyncManager.Join> = emptyList(),
    /** What the chip beside the date says about sync. */
    val sync: SyncBadge = SyncBadge.NotSynced,
)

/** Sync's state in a word or two, beside the date on Home. Tapping it opens Sync. */
enum class SyncBadge { Synced, Syncing, NotSynced, SignIn }

/**
 * Home: what's coming up, what's still fresh (new or due soon) and everyone.
 * On a phone "Everyone" is a row of photos; in the tablet/desktop list pane it's a list.
 */
@Composable
fun HomeScreen(
    model: HomeModel,
    asListPane: Boolean,
    selectedPersonId: Long?,
    themesOpen: Boolean,
    search: String?,
    onSearchChange: (String?) -> Unit,
    onToggleThemes: () -> Unit,
    onAddPerson: () -> Unit,
    onOpenSync: () -> Unit,
    onOpenPerson: (Long) -> Unit,
    onOpenCategory: (personId: Long, collectionId: Long) -> Unit,
    actions: NoteActions,
    peopleActions: PeopleActions,
    contentPadding: PaddingValues,
    modifier: Modifier = Modifier,
    onToggleFresh: () -> Unit = {},
) {
    val fresh = remember(model) { Insights.stillFresh(model.notes, model.people, model.collections) }
    val events = remember(model) { Insights.nextEvents(model.people, model.notes, model.collections, NEXT_EVENTS) }
    val peopleOrder = rememberReorderState<Long> { from, to -> peopleActions.onPlace(from, to) }
    val hPad = if (asListPane) 0.dp else 22.dp

    LazyColumn(
        modifier.fillMaxSize(),
        contentPadding = contentPadding,
    ) {
        item(key = "header") {
            Header(asListPane, themesOpen, search, onSearchChange, onToggleThemes, onAddPerson, model.sync, onOpenSync, Modifier.padding(horizontal = hPad))
        }

        if (search != null) {
            searchResults(model, search, actions, onOpenPerson, onOpenCategory, hPad)
            return@LazyColumn
        }

        items(model.joins, key = { "join-${it.fileId}" }) { join ->
            Box(Modifier.padding(start = hPad, end = hPad, top = 16.dp).animateItem()) { app.memi.ui.sheets.JoinCard(join) }
        }

        if (!asListPane) {
            item(key = "hero") {
                Box(Modifier.padding(start = hPad, end = hPad, top = 22.dp).animateItem()) {
                    when {
                        events.isNotEmpty() -> EventsTile(events) { u ->
                            val c = u.collection
                            if (c != null) onOpenCategory(u.person.id, c.id) else onOpenPerson(u.person.id)
                        }
                        model.people.isEmpty() -> WelcomeTile(onAddPerson)
                        else -> QuietTile(model.people.size, fresh.size)
                    }
                }
            }
        } else if (model.people.isEmpty()) {
            item(key = "welcome") { Box(Modifier.padding(top = 22.dp)) { WelcomeTile(onAddPerson) } }
        }

        if (fresh.isNotEmpty()) {
            item(key = "freshHeader") {
                FreshHeader(
                    collapsed = model.freshCollapsed,
                    hidden = fresh.size,
                    onToggle = onToggleFresh,
                    modifier = Modifier.padding(start = hPad, end = hPad, top = if (asListPane) 22.dp else 28.dp, bottom = 12.dp).animateItem(),
                )
            }
            if (!model.freshCollapsed) {
                items(fresh.take(if (asListPane) 5 else 6), key = { "fresh-${it.note.id}" }) { item ->
                    FreshCard(item, actions, onOpenCategory, Modifier.padding(horizontal = hPad).padding(bottom = 8.dp).animateItem())
                }
            }
        }

        if (model.people.isNotEmpty()) {
            item(key = "everyoneHeader") {
                SectionHeader("Everyone", Modifier.padding(start = hPad, end = hPad, top = 20.dp, bottom = 12.dp).animateItem(), count = model.people.size)
            }
            if (!asListPane) {
                item(key = "everyoneRow") {
                    LazyRow(
                        contentPadding = PaddingValues(horizontal = hPad),
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        modifier = Modifier.animateItem(),
                    ) {
                        items(model.people, key = { it.id }) { p ->
                            PersonCard(p, model.people, peopleActions, peopleOrder, Modifier.animateItem()) { onOpenPerson(p.id) }
                        }
                    }
                }
            } else {
                items(model.people, key = { "row-${it.id}" }) { p ->
                    PersonRow(
                        p,
                        Insights.openCount(p, model.notes),
                        selected = p.id == selectedPersonId,
                        modifier = Modifier.animateItem(),
                        menu = Pair(model.people, peopleActions),
                        reorder = peopleOrder,
                    ) { onOpenPerson(p.id) }
                }
            }
        }
    }
}

@Composable
private fun Header(
    asListPane: Boolean,
    themesOpen: Boolean,
    search: String?,
    onSearchChange: (String?) -> Unit,
    onToggleThemes: () -> Unit,
    onAddPerson: () -> Unit,
    sync: SyncBadge,
    onOpenSync: () -> Unit,
    modifier: Modifier,
) {
    val colors = Bento.colors
    Row(modifier.fillMaxWidth().padding(top = 28.dp), verticalAlignment = Alignment.CenterVertically) {
        AnimatedContent(
            targetState = search != null,
            transitionSpec = { fadeIn(Motion.soft()) togetherWith fadeOut(Motion.soft()) },
            label = "header",
            modifier = Modifier.weight(1f),
        ) { searching ->
            if (!searching) {
                Column {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                        if (!asListPane) {
                            // The chip is measured first; with very large text, the date gives way.
                            Text(
                                Dates.today().let { "${it.dayOfWeek.getDisplayName(java.time.format.TextStyle.SHORT, java.util.Locale.getDefault())} ${Dates.shortDate(it)}" },
                                style = BentoType.label, color = colors.muted, maxLines = 1, overflow = TextOverflow.Ellipsis,
                                modifier = Modifier.weight(1f, fill = false),
                            )
                        }
                        SyncChip(sync, onOpenSync)
                    }
                    Spacer(Modifier.height(4.dp))
                    FitText("Your people", style = if (asListPane) BentoType.display.copy(fontSize = BentoType.tileTitle.fontSize.times(1.27f)) else BentoType.display, color = colors.ink)
                }
            } else {
                val focus = remember { FocusRequester() }
                LaunchedEffect(Unit) { focus.requestFocus() }
                BasicTextField(
                    value = search.orEmpty(),
                    onValueChange = { onSearchChange(it) },
                    singleLine = true,
                    textStyle = BentoType.tileTitle.copy(color = colors.ink),
                    cursorBrush = SolidColor(colors.ink),
                    modifier = Modifier.fillMaxWidth().padding(end = 8.dp).focusRequester(focus),
                    decorationBox = { inner ->
                        Box(
                            Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(colors.card).padding(horizontal = 16.dp, vertical = 11.dp)
                        ) {
                            if (search.isNullOrEmpty()) Text("Search people and notes", style = BentoType.tileTitle, color = colors.muted, maxLines = 1)
                            inner()
                        }
                    },
                )
            }
        }
        Spacer(Modifier.width(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
            if (search == null) {
                HeaderButton(Icons.Rounded.Settings, "Settings", selected = themesOpen, onClick = onToggleThemes)
                HeaderButton(Icons.Rounded.Search, "Search") { onSearchChange("") }
                HeaderButton(Icons.Rounded.Add, "Add person", onClick = onAddPerson)
            } else {
                HeaderButton(Icons.Rounded.Close, "Close search") { onSearchChange(null) }
            }
        }
    }
}

private fun androidx.compose.foundation.lazy.LazyListScope.searchResults(
    model: HomeModel,
    query: String,
    actions: NoteActions,
    onOpenPerson: (Long) -> Unit,
    onOpenCategory: (Long, Long) -> Unit,
    hPad: androidx.compose.ui.unit.Dp,
) {
    val q = query.trim().lowercase()
    if (q.isEmpty()) return
    val people = model.people.filter { it.name.lowercase().contains(q) || it.relationship.lowercase().contains(q) }
    val byId = model.people.associateBy { it.id }
    val cols = model.collections.associateBy { it.id }
    val notes = model.notes.filter { it.text.lowercase().contains(q) }.take(40)
    if (people.isEmpty() && notes.isEmpty()) {
        item(key = "noResults") {
            Text("Nothing matches “$query”.", style = BentoType.body, color = Bento.colors.muted, modifier = Modifier.padding(start = hPad, end = hPad, top = 28.dp))
        }
        return
    }
    if (people.isNotEmpty()) {
        item(key = "sp") { SectionHeader("People", Modifier.padding(start = hPad, end = hPad, top = 24.dp, bottom = 8.dp)) }
        items(people, key = { "sp-${it.id}" }) { p ->
            PersonRow(p, Insights.openCount(p, model.notes), selected = false, modifier = Modifier.padding(horizontal = hPad)) { onOpenPerson(p.id) }
        }
    }
    if (notes.isNotEmpty()) {
        item(key = "sn") { SectionHeader("Notes", Modifier.padding(start = hPad, end = hPad, top = 24.dp, bottom = 12.dp)) }
        items(notes, key = { "sn-${it.id}" }) { n ->
            val person = byId[n.personId] ?: return@items
            NoteCard(
                note = n,
                title = n.text,
                subtitle = "${person.firstName} · ${cols[n.collectionId]?.name ?: ""}",
                actions = actions,
                leading = { PersonPhoto(person, Modifier.size(40.dp), RoundedCornerShape(13.dp)) },
                onClick = { onOpenCategory(person.id, n.collectionId) },
                modifier = Modifier.padding(horizontal = hPad).padding(bottom = 8.dp),
            )
        }
    }
}

@Composable
fun FreshCard(item: Insights.FreshItem, actions: NoteActions, onOpenCategory: (Long, Long) -> Unit, modifier: Modifier = Modifier) {
    val colors = Bento.colors
    val due = item.due
    val tag = if (due != null) {
        Triple("Due ${Dates.shortDayLabel(due)}", colors.gift, colors.giftInk)
    } else {
        Triple("New · ${Dates.agoShort(item.note.mentionedAt)}", colors.plan, colors.planInk)
    }
    NoteCard(
        note = item.note,
        title = item.note.text,
        subtitle = "${item.person.firstName} · ${item.collection?.name ?: "note"}",
        actions = actions,
        tag = tag,
        leading = { PersonPhoto(item.person, Modifier.size(44.dp), RoundedCornerShape(15.dp)) },
        onClick = { onOpenCategory(item.person.id, item.note.collectionId) },
        modifier = modifier,
    )
}

private const val NEXT_EVENTS = 3

/** The next few birthdays, dated categories and dated plans or notes, soonest first. */
@Composable
private fun EventsTile(events: List<Insights.Upcoming>, onOpen: (Insights.Upcoming) -> Unit) {
    val colors = Bento.colors
    Tile(Modifier.fillMaxWidth(), color = colors.gift, shape = RoundedCornerShape(28.dp), contentPadding = 14.dp) {
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            Row(Modifier.fillMaxWidth().padding(start = 6.dp, end = 2.dp, bottom = 4.dp), verticalAlignment = Alignment.CenterVertically) {
                Text("COMING UP", style = BentoType.caption, color = colors.giftInk, modifier = Modifier.weight(1f))
                Mascot(Modifier.size(32.dp))
            }
            events.forEach { u ->
                Row(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(18.dp)).clickable { onOpen(u) }.padding(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(
                        Modifier.width(62.dp).clip(RoundedCornerShape(14.dp)).background(colors.card).padding(vertical = 6.dp),
                        horizontalAlignment = Alignment.CenterHorizontally,
                    ) {
                        Text(Dates.shortDayLabel(u.date), style = BentoType.label, color = colors.ink, maxLines = 1)
                        Text(Dates.inDays(u.date), style = BentoType.caption, color = colors.muted, maxLines = 1)
                    }
                    Spacer(Modifier.width(12.dp))
                    Column(Modifier.weight(1f)) {
                        Text(u.title, style = BentoType.cardTitle, color = colors.ink, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(eventDetail(u), style = BentoType.label, color = colors.giftInk, maxLines = 1, overflow = TextOverflow.Ellipsis)
                    }
                    Spacer(Modifier.width(10.dp))
                    PersonPhoto(u.person, Modifier.size(40.dp), RoundedCornerShape(12.dp))
                }
            }
        }
    }
}

private fun eventDetail(u: Insights.Upcoming): String {
    val col = u.collection
    return when {
        u.note != null && col != null -> "${u.person.firstName} · ${col.name}"
        u.note != null -> u.person.firstName
        else -> {
            val what = col?.name?.lowercase() ?: "ideas"
            when (u.open) { 0 -> "Nothing in $what yet"; 1 -> "1 thing in $what"; else -> "${u.open} things in $what" }
        }
    }
}

/** "Still fresh", with a chevron to fold the list away (remembered). */
@Composable
private fun FreshHeader(collapsed: Boolean, hidden: Int, onToggle: () -> Unit, modifier: Modifier) {
    val colors = Bento.colors
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Row(
            Modifier.clip(RoundedCornerShape(12.dp)).clickable(onClick = onToggle).padding(end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text("Still fresh", style = BentoType.section, color = colors.ink)
            Spacer(Modifier.width(4.dp))
            Icon(
                if (collapsed) Icons.AutoMirrored.Rounded.KeyboardArrowRight else Icons.Rounded.KeyboardArrowDown,
                contentDescription = if (collapsed) "Show" else "Hide",
                tint = colors.muted,
                modifier = Modifier.size(22.dp),
            )
        }
        Spacer(Modifier.weight(1f))
        Text(if (collapsed) "$hidden hidden" else "new or due soon", style = BentoType.label, color = colors.muted)
    }
}

@Composable
private fun WelcomeTile(onAddPerson: () -> Unit) {
    val colors = Bento.colors
    Tile(Modifier.fillMaxWidth(), color = colors.gift, shape = RoundedCornerShape(28.dp), onClick = onAddPerson, contentPadding = 20.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Hi, I'm Memi", style = BentoType.tileTitle, color = colors.ink)
                Spacer(Modifier.height(6.dp))
                Text(
                    "Add someone you care about, then keep their gift ideas, plans and little things to remember in one place.",
                    style = BentoType.body, color = colors.giftInk,
                )
                Spacer(Modifier.height(14.dp))
                TagChip("+ Add a person", colors.card, colors.ink)
            }
            Spacer(Modifier.width(8.dp))
            Mascot(Modifier.size(104.dp))
        }
    }
}

@Composable
private fun QuietTile(people: Int, fresh: Int) {
    val colors = Bento.colors
    Tile(Modifier.fillMaxWidth(), color = colors.plan, shape = RoundedCornerShape(28.dp), contentPadding = 20.dp) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(Modifier.weight(1f)) {
                Text("Nothing coming up", style = BentoType.tileTitle, color = colors.ink)
                Spacer(Modifier.height(6.dp))
                Text(
                    if (fresh == 0) "All caught up across $people ${if (people == 1) "person" else "people"}." else "$fresh things still fresh below.",
                    style = BentoType.body, color = colors.planInk,
                )
            }
            Mascot(Modifier.size(84.dp))
        }
    }
}

/** Memi the elephant (Microsoft Fluent Emoji, MIT). Bobs gently, as if breathing. */
@Composable
fun Mascot(modifier: Modifier = Modifier) {
    val bob = rememberInfiniteTransition(label = "memi")
    // Read in the draw phase, so the gentle bob never recomposes anything.
    val y = bob.animateFloat(
        initialValue = 0f,
        targetValue = -4f,
        animationSpec = infiniteRepeatable(tween(1800, easing = Motion.CozyEase), RepeatMode.Reverse),
        label = "bob",
    )
    Image(
        painter = app.memi.platform.mascotPainter(),
        contentDescription = "Memi the elephant",
        modifier = modifier.graphicsLayer { translationY = y.value * density },
    )
}

@Composable
private fun PersonCard(p: Person, all: List<Person>, actions: PeopleActions, order: ReorderState<Long>, modifier: Modifier, onClick: () -> Unit) {
    val colors = Bento.colors
    val interaction = remember { MutableInteractionSource() }
    var menu by remember { mutableStateOf(false) }
    Box(modifier) {
        Column(
            Modifier.width(82.dp).reorderable(order, p.id, RoundedCornerShape(22.dp))
                .pressScale(interaction, 0.95f).pressable(onClick, { menu = true }, interaction, order.handle(p.id)),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Box {
                PersonPhoto(p, Modifier.width(82.dp).height(92.dp), RoundedCornerShape(22.dp))
                if (p.pinned) PinBadge(Modifier.align(Alignment.TopEnd).padding(6.dp))
            }
            Spacer(Modifier.height(6.dp))
            OneLine(p.firstName, BentoType.label.copy(fontWeight = androidx.compose.ui.text.font.FontWeight.Bold), colors.ink)
        }
        PersonMenu(menu, { menu = false }, p, all, actions)
    }
}

@Composable
fun PersonRow(
    p: Person,
    open: Int,
    selected: Boolean,
    modifier: Modifier = Modifier,
    menu: Pair<List<Person>, PeopleActions>? = null,
    /** Set to let people be dragged into a new order. */
    reorder: ReorderState<Long>? = null,
    onClick: () -> Unit,
) {
    val colors = Bento.colors
    var menuOpen by remember { mutableStateOf(false) }
    Box(modifier) {
        val shape = RoundedCornerShape(16.dp)
        Tile(
            Modifier.fillMaxWidth().padding(bottom = 2.dp)
                .then(if (reorder != null) Modifier.reorderable(reorder, p.id, shape) else Modifier),
            color = if (selected) colors.card else if (reorder?.dragging == p.id) colors.sheet else Color.Transparent,
            shape = shape,
            onClick = onClick,
            onMenu = if (menu != null) ({ menuOpen = true }) else null,
            contentPadding = 6.dp,
            reorder = reorder?.handle(p.id),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(horizontal = 4.dp)) {
                PersonPhoto(p, Modifier.size(44.dp), RoundedCornerShape(14.dp))
                Spacer(Modifier.width(12.dp))
                OneLine(p.name, BentoType.itemTitle, colors.ink, Modifier.weight(1f))
                if (p.pinned) PinBadge(Modifier.padding(end = 8.dp))
                if (open > 0) Text("$open open", style = BentoType.caption, color = colors.muted)
            }
        }
        if (menu != null) PersonMenu(menuOpen, { menuOpen = false }, p, menu.first, menu.second)
    }
}

@Composable
private fun PinBadge(modifier: Modifier = Modifier) {
    val colors = Bento.colors
    Box(
        modifier.size(22.dp).clip(RoundedCornerShape(8.dp)).background(colors.sheet),
        contentAlignment = Alignment.Center,
    ) {
        Icon(Icons.Rounded.PushPin, contentDescription = "Pinned", tint = colors.ink, modifier = Modifier.size(13.dp))
    }
}

/** Pin, reorder, edit or remove someone. Moving only happens within the pinned or unpinned group. */
@Composable
private fun PersonMenu(expanded: Boolean, onDismiss: () -> Unit, p: Person, all: List<Person>, actions: PeopleActions) {
    val colors = Bento.colors
    val group = all.filter { it.pinned == p.pinned }
    val index = group.indexOfFirst { it.id == p.id }
    BentoMenu(expanded, onDismiss) {
        Column(Modifier.padding(horizontal = 8.dp)) {
            MenuRow(if (p.pinned) "Unpin" else "Pin to the front", Icons.Rounded.PushPin) { onDismiss(); actions.onPin(p, !p.pinned) }
            if (index > 0) MenuRow("Move earlier", Icons.AutoMirrored.Rounded.ArrowBack) { onDismiss(); actions.onMove(p, -1) }
            if (index in 0 until group.lastIndex) MenuRow("Move later", Icons.AutoMirrored.Rounded.ArrowForward) { onDismiss(); actions.onMove(p, 1) }
            MenuRow("Edit", Icons.Rounded.Edit) { onDismiss(); actions.onEdit(p) }
            MenuRow("Remove", Icons.Rounded.DeleteOutline, tint = colors.danger) { onDismiss(); actions.onRemove(p) }
        }
    }
}

/**
 * The sync chip beside the date: Synced, Syncing…, Not synced or Sign in. "Syncing…" shows only
 * when a sync takes more than a moment, so the regular checks every few seconds don't flicker.
 */
@Composable
private fun SyncChip(badge: SyncBadge, onClick: () -> Unit) {
    val colors = Bento.colors
    var shown by remember { mutableStateOf(if (badge == SyncBadge.Syncing) SyncBadge.Synced else badge) }
    LaunchedEffect(badge) {
        if (badge == SyncBadge.Syncing) kotlinx.coroutines.delay(800)
        shown = badge
    }
    val (icon, label) = when (shown) {
        SyncBadge.Synced -> Icons.Rounded.CloudDone to "Synced"
        SyncBadge.Syncing -> Icons.Rounded.Sync to "Syncing…"
        SyncBadge.NotSynced -> Icons.Rounded.CloudOff to "Not synced"
        SyncBadge.SignIn -> Icons.Rounded.ErrorOutline to "Sign in"
    }
    val (bg, ink) = when (shown) {
        SyncBadge.Synced -> colors.plan to colors.planInk
        SyncBadge.Syncing -> colors.tiles[3] to colors.tileInks[3]
        SyncBadge.NotSynced -> colors.card to colors.ink
        SyncBadge.SignIn -> colors.gift to colors.giftInk
    }
    Row(
        Modifier
            .clip(CircleShape)
            .background(bg)
            .semantics(mergeDescendants = true) { contentDescription = "Sync: $label. Open Sync" }
            .clickable(role = Role.Button, onClick = onClick)
            .padding(start = 8.dp, end = 11.dp, top = 4.dp, bottom = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp),
    ) {
        Icon(icon, contentDescription = null, tint = ink, modifier = Modifier.size(15.dp))
        Text(label, style = BentoType.label, color = ink, maxLines = 1, softWrap = false)
    }
}
