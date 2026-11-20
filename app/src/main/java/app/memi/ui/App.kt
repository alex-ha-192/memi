package app.memi.ui

import app.memi.ui.screens.FirstLaunch
import app.memi.ui.screens.SyncBadge

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.ContentTransform
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.slideOutVertically
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Person
import app.memi.ui.components.NoteActions
import app.memi.ui.components.PointerModeProvider
import app.memi.ui.components.ThemePanel
import app.memi.ui.components.ToastPill
import app.memi.ui.screens.CollectionAction
import app.memi.ui.screens.CollectionSheet
import app.memi.ui.screens.HomeModel
import app.memi.ui.screens.HomeScreen
import app.memi.ui.screens.PeopleActions
import app.memi.ui.screens.PersonCallbacks
import app.memi.ui.screens.PersonScreen
import app.memi.ui.sheets.CategoryEditorSheet
import app.memi.ui.sheets.SyncSheet
import app.memi.ui.sheets.NoteEditorSheet
import app.memi.ui.sheets.PersonEditorSheet
import app.memi.ui.sheets.PhotoTools
import app.memi.platform.PlatformBackHandler
import app.memi.platform.PlatformSystemBars
import app.memi.sync.SyncManager
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoTheme
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion

private val TwoPaneWidth = 840.dp
private val ListPaneWidth = 400.dp

private sealed interface Dialog {
    data class EditPerson(val person: Person?) : Dialog
    data class RemovePerson(val person: Person) : Dialog
    data class EditNote(val note: Note) : Dialog
    data class Collection(val personId: Long, val collectionId: Long) : Dialog
    data class EditCollection(val personId: Long, val collection: NoteCollection?) : Dialog
    data class RemoveCollection(val collection: NoteCollection) : Dialog
    data object Sync : Dialog
}

/** The sync engine, for the Sync settings sheet and the status line in the theme panel. */
val LocalSync = staticCompositionLocalOf<SyncManager> { error("No sync provided") }

@Composable
fun MemiRoot(vm: AppViewModel, sync: SyncManager) {
    val theme by vm.theme.collectAsStateWithLifecycle()
    CompositionLocalProvider(LocalSync provides sync) {
        BentoTheme(theme) {
            PlatformSystemBars(dark = Bento.colors.isDark)
            val welcomed by vm.welcomed.collectAsStateWithLifecycle()
            val people by vm.people.collectAsStateWithLifecycle()
            val syncState by sync.state.collectAsStateWithLifecycle()
            // A device that already has people or sync (every install from before 0.6.2) skips the questions.
            LaunchedEffect(welcomed, people.isEmpty(), syncState.enabled) {
                if (welcomed == false && (people.isNotEmpty() || syncState.enabled)) vm.setWelcomed()
            }
            when {
                welcomed == null -> Box(Modifier.fillMaxSize().background(Bento.colors.bg))
                welcomed == false && people.isEmpty() && !syncState.enabled -> FirstLaunch { token ->
                    vm.setWelcomed()
                    if (token != null) {
                        sync.launch { connect(token) }
                        vm.requestSyncSheet()
                    }
                }
                else -> PointerModeProvider { Content(vm) }
            }
        }
    }
}

@Composable
private fun Content(vm: AppViewModel) {
    val colors = Bento.colors
    val people by vm.people.collectAsStateWithLifecycle()
    val notes by vm.notes.collectAsStateWithLifecycle()
    val collections by vm.collections.collectAsStateWithLifecycle()
    val theme by vm.theme.collectAsStateWithLifecycle()
    val toast by vm.toast.collectAsStateWithLifecycle()

    var selectedId by rememberSaveable { mutableStateOf<Long?>(null) }
    val syncState by LocalSync.current.state.collectAsStateWithLifecycle()
    var themesOpen by rememberSaveable { mutableStateOf(false) }
    var search by rememberSaveable { mutableStateOf<String?>(null) }
    var dialog by remember { mutableStateOf<Dialog?>(null) }

    // Tapping anywhere outside the theme panel closes it. The tap still goes through to whatever
    // was under it; the palette button ignores the tap that just closed the panel.
    var panelBounds by remember { mutableStateOf(Rect.Zero) }
    var closedAt by remember { mutableLongStateOf(0L) }
    val isOpen by rememberUpdatedState(themesOpen)
    val toggleThemes = {
        if (themesOpen) themesOpen = false
        else if (System.currentTimeMillis() - closedAt > 400) themesOpen = true
    }

    // If the selected person is removed, go back home.
    LaunchedEffect(people, selectedId) {
        if (selectedId != null && people.isNotEmpty() && people.none { it.id == selectedId }) selectedId = null
    }

    val freshCollapsed by vm.freshCollapsed.collectAsStateWithLifecycle()
    val reminders by vm.birthdayReminders.collectAsStateWithLifecycle()
    val context = androidx.compose.ui.platform.LocalContext.current
    val syncBadge = when {
        syncState.problem is app.memi.sync.SyncManager.Problem.NeedsSignIn -> SyncBadge.SignIn
        syncState.busy -> SyncBadge.Syncing
        syncState.enabled && syncState.problem == null -> SyncBadge.Synced
        else -> SyncBadge.NotSynced
    }
    val model = remember(people, notes, collections, freshCollapsed, syncState.joins, syncBadge) {
        HomeModel(people, notes, collections, freshCollapsed, syncState.joins, syncBadge)
    }
    val actions = remember(collections) {
        NoteActions(
            collections = collections,
            onDone = { n, o -> vm.markDone(n, o) },
            onReopen = { vm.reopen(it) },
            onMove = { n, c -> vm.moveNote(n, c) },
            onEdit = { dialog = Dialog.EditNote(it) },
            onDelete = { vm.deleteNote(it) },
        )
    }
    val peopleActions = remember {
        PeopleActions(
            onPin = { p, pin -> vm.setPinned(p, pin) },
            onMove = { p, d -> vm.movePerson(p, d) },
            onEdit = { dialog = Dialog.EditPerson(it) },
            onRemove = { dialog = Dialog.RemovePerson(it) },
            onPlace = { a, b -> vm.placePerson(a, b) },
        )
    }
    fun callbacksFor(person: Person) = PersonCallbacks(
        onEditPerson = { dialog = Dialog.EditPerson(person) },
        onRemovePerson = { dialog = Dialog.RemovePerson(person) },
        onOpenCollection = { c -> dialog = Dialog.Collection(person.id, c.id) },
        onCollectionMenu = { c, action ->
            when (action) {
                CollectionAction.EDIT -> dialog = Dialog.EditCollection(person.id, c)
                CollectionAction.MOVE_EARLIER -> vm.moveCollection(c, -1)
                CollectionAction.MOVE_LATER -> vm.moveCollection(c, 1)
                CollectionAction.DELETE -> dialog = Dialog.RemoveCollection(c)
            }
        },
        onNewCollection = { dialog = Dialog.EditCollection(person.id, null) },
        onPlaceCollection = { a, b -> vm.placeCollection(a, b) },
    )

    // Opening something from Still fresh or search goes straight to its category.
    val openCategory: (Long, Long) -> Unit = { personId, collectionId ->
        selectedId = personId
        search = null
        dialog = Dialog.Collection(personId, collectionId)
    }

    // Right after signing in on first launch: the Sync sheet shows how it's going (and the code
    // to compare, if another device has the key).
    val openSync by vm.openSync.collectAsStateWithLifecycle()
    LaunchedEffect(openSync) { if (openSync) { dialog = Dialog.Sync; vm.syncSheetOpened() } }

    PlatformBackHandler(themesOpen) { themesOpen = false }
    PlatformBackHandler(search != null && !themesOpen) { search = null }

    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()

    BoxWithConstraints(
        Modifier
            .fillMaxSize()
            .background(colors.bg)
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                        if (isOpen && event.type == PointerEventType.Press) {
                            val pos = event.changes.firstOrNull()?.position ?: continue
                            if (!panelBounds.contains(pos)) {
                                themesOpen = false
                                closedAt = System.currentTimeMillis()
                            }
                        }
                    }
                }
            }
    ) {
        val twoPane = maxWidth >= TwoPaneWidth
        val bottomPadding = navBottom + 96.dp

        if (twoPane) {
            val shown = people.firstOrNull { it.id == selectedId } ?: people.firstOrNull()
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.width(ListPaneWidth).fillMaxHeight()) {
                    HomeScreen(
                        model = model, asListPane = true, selectedPersonId = shown?.id,
                        themesOpen = themesOpen, search = search, onSearchChange = { search = it },
                        onToggleThemes = toggleThemes,
                        onAddPerson = { dialog = Dialog.EditPerson(null) },
                        onOpenSync = { dialog = Dialog.Sync },
                        onOpenPerson = { selectedId = it; search = null },
                        onOpenCategory = openCategory,
                        actions = actions, peopleActions = peopleActions, onToggleFresh = { vm.toggleFresh() },
                        contentPadding = PaddingValues(start = 28.dp, end = 20.dp, top = statusTop, bottom = bottomPadding),
                    )
                }
                Box(Modifier.width(1.dp).fillMaxHeight().background(colors.line))
                Box(Modifier.weight(1f).fillMaxHeight()) {
                    AnimatedContent(
                        targetState = shown,
                        contentKey = { it?.id },
                        transitionSpec = { fadeIn(Motion.soft()) togetherWith fadeOut(Motion.soft()) },
                        label = "detail",
                    ) { person ->
                        if (person != null) {
                            PersonScreen(
                                person = person, notes = notes, collections = collections,
                                actions = actions, callbacks = callbacksFor(person),
                                contentPadding = PaddingValues(start = 28.dp, end = 28.dp, top = statusTop + 28.dp, bottom = bottomPadding),
                            )
                        }
                    }
                }
            }
        } else {
            PlatformBackHandler(selectedId != null && !themesOpen) { selectedId = null }
            val current = people.firstOrNull { it.id == selectedId }
            AnimatedContent(
                targetState = current,
                contentKey = { it?.id },
                transitionSpec = { phoneTransition(forward = targetState != null) },
                label = "phone",
            ) { person ->
                val padding = PaddingValues(top = statusTop, bottom = bottomPadding)
                if (person == null) {
                    HomeScreen(
                        model = model, asListPane = false, selectedPersonId = null,
                        themesOpen = themesOpen, search = search, onSearchChange = { search = it },
                        onToggleThemes = toggleThemes,
                        onAddPerson = { dialog = Dialog.EditPerson(null) },
                        onOpenSync = { dialog = Dialog.Sync },
                        onOpenPerson = { selectedId = it; search = null },
                        onOpenCategory = openCategory,
                        actions = actions, peopleActions = peopleActions, onToggleFresh = { vm.toggleFresh() },
                        contentPadding = padding,
                    )
                } else {
                    PersonScreen(
                        person = person, notes = notes, collections = collections,
                        actions = actions, callbacks = callbacksFor(person),
                        contentPadding = padding,
                    )
                }
            }
        }

        // Undo and other short messages.
        AnimatedVisibility(
            visible = toast != null && !themesOpen,
            enter = fadeIn(Motion.soft()) + slideInVertically(Motion.settle()) { it / 2 },
            exit = fadeOut(Motion.soft()) + slideOutVertically(Motion.soft()) { it / 2 },
            modifier = Modifier.align(Alignment.BottomCenter).navigationBarsPadding().padding(bottom = 20.dp, start = 16.dp, end = 16.dp),
        ) {
            toast?.let { t -> ToastPill(t) { t.action?.invoke(); vm.dismissToast() } }
        }

        // Themes: slides up without dimming. Tap outside, the palette button or back to close.
        AnimatedVisibility(
            visible = themesOpen,
            enter = slideInVertically(Motion.settle()) { it } + fadeIn(Motion.soft()),
            exit = slideOutVertically(Motion.soft()) { it } + fadeOut(Motion.soft()),
            modifier = Modifier.align(if (twoPane) Alignment.BottomStart else Alignment.BottomCenter),
        ) {
            ThemePanel(
                choice = theme,
                onScheme = vm::setScheme,
                onAppearance = vm::setAppearance,
                syncDetail = when {
                    syncState.problem != null && syncState.enabled -> "Needs a look"
                    syncState.enabled -> syncState.account ?: "On"
                    else -> "Off"
                },
                onSync = { themesOpen = false; dialog = Dialog.Sync },
                reminders = reminders,
                onReminders = { vm.setBirthdayReminders(context, it) },
                modifier = (if (twoPane) Modifier.width(ListPaneWidth) else Modifier.fillMaxWidth())
                    .onGloballyPositioned { panelBounds = it.boundsInRoot() }
                    .navigationBarsPadding(),
            )
        }
    }

    when (val d = dialog) {
        is Dialog.EditPerson -> PersonEditorSheet(
            initial = d.person,
            onDismiss = { dialog = null },
            photos = remember {
                PhotoTools(
                    loadFile = { vm.loadPhotoFile(it) },
                    saveCrop = { image, crop -> vm.savePhoto(image, crop) },
                    discard = { vm.discardPhoto(it) },
                )
            },
            onSave = { p ->
                val adding = d.person == null
                vm.savePerson(p) { id -> if (adding) selectedId = id }
                dialog = null
            },
            onDelete = { p -> dialog = Dialog.RemovePerson(p) },
        )
        is Dialog.RemovePerson -> ConfirmDialog(
            title = "Remove ${d.person.firstName}?",
            body = "Their notes and photo will be deleted from this phone. This can't be undone.",
            confirm = "Remove",
            onDismiss = { dialog = null },
            onConfirm = {
                vm.deletePerson(d.person)
                if (selectedId == d.person.id) selectedId = null
                dialog = null
            },
        )
        is Dialog.EditNote -> NoteEditorSheet(
            note = d.note, collections = collections.filter { it.personId == d.note.personId },
            onDismiss = { dialog = null },
            onSave = { vm.updateNote(it); dialog = null },
        )
        is Dialog.Collection -> {
            val person = people.firstOrNull { it.id == d.personId }
            val c = collections.firstOrNull { it.id == d.collectionId }
            if (person != null && c != null) {
                CollectionSheet(
                    person, c, notes, actions,
                    onAdd = { text -> vm.addNote(person, c, text, null) },
                    onDismiss = { dialog = null },
                    onPlace = { a, b -> vm.placeNote(a, b) },
                )
            } else {
                LaunchedEffect(Unit) { dialog = null }
            }
        }
        is Dialog.EditCollection -> {
            val person = people.firstOrNull { it.id == d.personId }
            if (person != null) {
                CategoryEditorSheet(
                    person = person, initial = d.collection,
                    onDismiss = { dialog = null },
                    onSave = { name, occasion, day, yearly ->
                        val c = d.collection
                        if (c == null) vm.addCollection(person, name, occasion, day, yearly)
                        else vm.updateCollection(c.copy(name = name, occasion = occasion, occasionDay = day, yearly = yearly))
                        dialog = null
                    },
                )
            } else {
                LaunchedEffect(Unit) { dialog = null }
            }
        }
        is Dialog.RemoveCollection -> ConfirmDialog(
            title = "Remove ${d.collection.name}?",
            body = "Anything in it moves to another of their categories, so nothing is lost.",
            confirm = "Remove",
            onDismiss = { dialog = null },
            onConfirm = { vm.deleteCollection(d.collection); dialog = null },
        )
        Dialog.Sync -> SyncSheet(onDismiss = { dialog = null })
        null -> Unit
    }
}

@Composable
internal fun ConfirmDialog(title: String, body: String, confirm: String, onDismiss: () -> Unit, onConfirm: () -> Unit, dismiss: String = "Keep") {
    val colors = Bento.colors
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = colors.sheet,
        title = { Text(title, style = BentoType.section, color = colors.ink) },
        text = { Text(body, style = BentoType.body, color = colors.muted) },
        confirmButton = { TextButton(onClick = onConfirm) { Text(confirm, style = BentoType.itemTitle, color = colors.danger) } },
        dismissButton = { TextButton(onClick = onDismiss) { Text(dismiss, style = BentoType.itemTitle, color = colors.ink) } },
    )
}

private fun phoneTransition(forward: Boolean): ContentTransform =
    if (forward) {
        (slideInHorizontally(Motion.settle()) { it / 6 } + fadeIn(Motion.soft())) togetherWith fadeOut(Motion.soft())
    } else {
        fadeIn(Motion.soft()) togetherWith (slideOutHorizontally(Motion.settle()) { it / 6 } + fadeOut(Motion.soft()))
    }
