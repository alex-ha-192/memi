package app.memi.ui

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.unit.IntRect
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.memi.data.Repository
import app.memi.data.ThemePrefs
import app.memi.platform.PlatformPhotos
import app.memi.data.Appearance
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Outcome
import app.memi.data.Person
import app.memi.data.ThemeChoice
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

data class Toast(val id: Long, val message: String, val actionLabel: String? = null, val action: (() -> Unit)? = null)

class AppViewModel(private val repo: Repository, private val prefs: ThemePrefs) : ViewModel() {

    val people: StateFlow<List<Person>> = repo.people.stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())
    val collections: StateFlow<List<NoteCollection>> = repo.collections.stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())
    val notes: StateFlow<List<Note>> = repo.notes.stateIn(viewModelScope, SharingStarted.Eagerly, emptyList())
    val theme: StateFlow<ThemeChoice> = prefs.choice.stateIn(viewModelScope, SharingStarted.Eagerly, ThemeChoice())
    val freshCollapsed: StateFlow<Boolean> = prefs.freshCollapsed.stateIn(viewModelScope, SharingStarted.Eagerly, false)

    fun toggleFresh() = viewModelScope.launch { prefs.setFreshCollapsed(!freshCollapsed.value) }

    /** Null until settings have loaded, then whether the first-launch questions are done. */
    val welcomed: StateFlow<Boolean?> = prefs.welcomed.stateIn(viewModelScope, SharingStarted.Eagerly, null)
    fun setWelcomed() = viewModelScope.launch { prefs.setWelcomed() }

    /** Set when the Sync sheet should open by itself (right after signing in on first launch). */
    private val _openSync = MutableStateFlow(false)
    val openSync: StateFlow<Boolean> = _openSync.asStateFlow()
    fun requestSyncSheet() { _openSync.value = true }
    fun syncSheetOpened() { _openSync.value = false }

    /** Days before a birthday to be reminded (0, 1, 7); empty is off. */
    val birthdayReminders: StateFlow<Set<Int>> = prefs.birthdayReminders.stateIn(viewModelScope, SharingStarted.Eagerly, emptySet())

    fun setBirthdayReminders(context: android.content.Context, days: Set<Int>) = viewModelScope.launch {
        prefs.setBirthdayReminders(days)
        app.memi.platform.BirthdayReminders.schedule(context.applicationContext, days.isNotEmpty())
        app.memi.platform.BirthdayReminders.check(context.applicationContext)
    }

    private val _toast = MutableStateFlow<Toast?>(null)
    val toast: StateFlow<Toast?> = _toast.asStateFlow()
    private var toastJob: Job? = null

    // --- Notes ------------------------------------------------------------------------------

    fun addNote(person: Person, collection: NoteCollection, text: String, dueAt: Long?) {
        if (text.isBlank()) return
        viewModelScope.launch {
            val id = repo.addNote(person.id, collection.id, text, dueAt)
            showToast("Added to ${collection.name}", "Undo") { viewModelScope.launch { repo.deleteNoteIds(listOf(id)) } }
        }
    }

    fun markDone(note: Note, outcome: Int = Outcome.NONE) = viewModelScope.launch {
        repo.markDone(note, outcome)
        val label = when (outcome) {
            Outcome.HIT -> "Marked as a hit"
            Outcome.MISS -> "Marked as a miss"
            else -> "Marked done"
        }
        showToast(label, "Undo") { viewModelScope.launch { repo.updateNote(note) } }
    }

    fun reopen(note: Note) = viewModelScope.launch { repo.reopen(note) }

    // Drag and drop
    fun placePerson(id: Long, target: Long) = viewModelScope.launch { repo.placePerson(id, target) }
    fun placeCollection(id: Long, target: Long) = viewModelScope.launch { repo.placeCollection(id, target) }
    fun placeNote(id: Long, target: Long) = viewModelScope.launch { repo.placeNote(id, target) }

    fun moveNote(note: Note, to: NoteCollection) = viewModelScope.launch {
        repo.updateNote(note.copy(collectionId = to.id))
        showToast("Moved to ${to.name}", "Undo") { viewModelScope.launch { repo.updateNote(note) } }
    }

    fun updateNote(note: Note) = viewModelScope.launch { repo.updateNote(note) }

    fun deleteNote(note: Note) = viewModelScope.launch {
        repo.deleteNote(note)
        showToast("Deleted", "Undo") { viewModelScope.launch { repo.restoreNote(note) } }
    }

    // --- People -----------------------------------------------------------------------------

    fun savePerson(person: Person, onSaved: (Long) -> Unit = {}) = viewModelScope.launch {
        if (person.id == 0L) onSaved(repo.addPerson(person))
        else { repo.updatePerson(person); onSaved(person.id) }
    }

    fun deletePerson(person: Person) = viewModelScope.launch {
        repo.deletePerson(person)
        showToast("Removed ${person.firstName}")
    }

    fun setPinned(person: Person, pinned: Boolean) = viewModelScope.launch {
        repo.setPinned(person, pinned)
        showToast(if (pinned) "Pinned ${person.firstName}" else "Unpinned ${person.firstName}")
    }

    fun movePerson(person: Person, delta: Int) = viewModelScope.launch { repo.movePerson(person, delta) }

    /** Gives someone another of the theme's colours (it follows them to every device). */
    fun setTone(person: Person, tone: Int) = viewModelScope.launch { repo.updatePerson(person.copy(toneIndex = tone)) }


    suspend fun loadPhotoFile(path: String): ImageBitmap? = PlatformPhotos.loadFile(path)

    suspend fun savePhoto(image: ImageBitmap, crop: IntRect): String? = PlatformPhotos.saveCrop(image, crop)

    fun discardPhoto(path: String?) = repo.deletePhotoFile(path)

    // --- Categories (per person) --------------------------------------------------------------

    fun addCollection(person: Person, name: String, occasion: String, occasionDay: Long?, yearly: Boolean) = viewModelScope.launch {
        if (name.isNotBlank()) repo.addCollection(person.id, name, occasion, occasionDay, yearly)
    }

    fun updateCollection(c: NoteCollection) = viewModelScope.launch { if (c.name.isNotBlank()) repo.updateCollection(c) }

    fun moveCollection(c: NoteCollection, delta: Int) = viewModelScope.launch { repo.moveCollection(c, delta) }

    fun deleteCollection(c: NoteCollection) = viewModelScope.launch {
        repo.deleteCollection(c)
        showToast("Removed ${c.name}")
    }

    // --- Theme ------------------------------------------------------------------------------

    fun setScheme(name: String) = viewModelScope.launch { prefs.setScheme(name) }

    fun setAppearance(a: Appearance) = viewModelScope.launch { prefs.setAppearance(a) }

    // --- Backup file ------------------------------------------------------------------------

    fun saveBackup(context: android.content.Context, uri: android.net.Uri) = viewModelScope.launch {
        val result = runCatching {
            val bytes = repo.exportFile()
            kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                context.contentResolver.openOutputStream(uri, "wt")!!.use { it.write(bytes) }
            }
        }
        showToast(result.fold({ "Backup saved. It isn't encrypted, so keep it somewhere private." }, { "Couldn't save the backup: ${it.message}" }))
    }

    fun openBackup(context: android.content.Context, uri: android.net.Uri) = viewModelScope.launch {
        val result = runCatching {
            val bytes = kotlinx.coroutines.withContext(kotlinx.coroutines.Dispatchers.IO) {
                context.contentResolver.openInputStream(uri)!!.use { it.readBytes() }
            }
            repo.importFile(bytes)
        }
        showToast(result.getOrElse { "Couldn't open that backup: ${it.message}" })
    }

    // --- Toasts -----------------------------------------------------------------------------

    fun showToast(message: String, actionLabel: String? = null, action: (() -> Unit)? = null) {
        val t = Toast(System.nanoTime(), message, actionLabel, action)
        _toast.value = t
        toastJob?.cancel()
        toastJob = viewModelScope.launch {
            delay(4000)
            if (_toast.value?.id == t.id) _toast.value = null
        }
    }

    fun dismissToast() {
        _toast.value = null
    }
}
