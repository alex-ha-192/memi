package app.memi.sync

import app.memi.data.AppDatabase
import app.memi.data.Backup
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Keeps an encrypted copy of everything in the Google account you pick, without any further
 * fuss. Changes are uploaded a few seconds after you make them (and when you leave the app), and
 * changes made on another phone are pulled in when you open Memi.
 *
 * It's one encrypted file, so if two phones change things at the same moment, the one that
 * uploads last wins. For one person's phone plus the odd tablet, that's plenty.
 */
class SyncManager(private val platform: SyncPlatform, private val db: AppDatabase, private val scope: CoroutineScope) {

    sealed interface Problem {
        /** Google wants you to sign in again (access was removed, or the account changed). */
        data object NeedsSignIn : Problem
        /** There's a backup but this phone doesn't have its key (not set up from your old phone). */
        data object NeedsRecoveryKey : Problem
        /** Both this phone and the account have people in them; you choose which to keep. */
        data object Conflict : Problem
        data class Failed(val message: String) : Problem
    }

    data class State(
        val enabled: Boolean = false,
        val account: String? = null,
        val busy: Boolean = false,
        val lastSyncedAt: Long? = null,
        val problem: Problem? = null,
        val keyFollows: Boolean = false,
    )

    private val prefs = platform.settings
    private val vault = platform.keys
    private val mutex = Mutex()
    private var pushJob: Job? = null
    private var ignoreChangesUntil = 0L
    private var lastForeground = 0L
    /** Kept between "connect" and the person answering a recovery-key or conflict question. */
    private var pendingToken: String? = null

    private val _state = MutableStateFlow(
        State(
            enabled = prefs.getBoolean(ENABLED, false),
            account = prefs.getString(ACCOUNT),
            lastSyncedAt = prefs.getLong(LAST_SYNC, 0L).takeIf { it > 0 },
        )
    )
    val state: StateFlow<State> = _state.asStateFlow()

    init {
        if (_state.value.enabled) scope.launch { _state.update { it.copy(keyFollows = vault.followsToNewDevice()) } }
    }

    // --- Turning it on ------------------------------------------------------------------------

    /** Called with a token right after you've picked your Google account. */
    suspend fun connect(token: String) = guarded("connect") {
        pendingToken = token
        val drive = DriveClient(token)
        val email = drive.email()
        prefs.putString(ACCOUNT, email)
        _state.update { it.copy(account = email) }

        val remote = drive.find(FILE)
        val key = vault.local() ?: vault.fromCloud()
        when {
            remote == null -> {
                vault.save(key ?: Crypto.newKey())
                enable()
                pushNow(drive)
            }
            key == null -> _state.update { it.copy(problem = Problem.NeedsRecoveryKey) }
            db.people().all().isEmpty() -> {
                vault.save(key)
                restoreFrom(drive, key)
                enable()
            }
            else -> {
                vault.save(key)
                _state.update { it.copy(problem = Problem.Conflict) }
            }
        }
    }

    suspend fun useRecoveryKey(text: String): Boolean {
        val key = Crypto.keyFromText(text) ?: return false
        return adoptKey(key)
    }

    /** Checks [key] really opens the backup in your account, then keeps it and finishes connecting. */
    private suspend fun adoptKey(key: ByteArray): Boolean {
        val token = pendingToken ?: platform.silentToken() ?: return false
        val ok = runCatching {
            val drive = DriveClient(token)
            val remote = drive.find(FILE) ?: return false
            Crypto.open(key, drive.download(remote.id))
        }.isSuccess
        if (ok) {
            vault.save(key)
            _state.update { it.copy(problem = null) }
            connect(token)
        }
        return ok
    }

    // --- Giving a computer the key by QR code (see Pairing.kt) --------------------------------

    /** On the phone: sends this phone's key to the computer whose code was scanned. */
    suspend fun sendKeyTo(code: String): String {
        val scanned = Pairing.parse(code) ?: return "That isn't a Your Memi code. On the computer, open Sync to show one."
        val key = vault.local() ?: return "Turn on sync on this phone first."
        val token = platform.silentToken() ?: return "Sign in to Google again in Sync first."
        return runCatching {
            DriveClient(token).create(scanned.fileName, Pairing.sealFor(scanned, key))
            "Sent. Your computer will finish setting up in a few seconds."
        }.getOrElse { "Couldn't reach Google Drive: ${it.message}" }
    }

    /** Answers [Problem.Conflict]: keep what's in your Google account, or what's on this phone. */
    suspend fun resolveConflict(keepAccount: Boolean) = guarded("resolve") {
        val token = pendingToken ?: throw IllegalStateException("Sign in again")
        val drive = DriveClient(token)
        val key = vault.local() ?: throw IllegalStateException("No key")
        if (keepAccount) restoreFrom(drive, key) else pushNow(drive)
        enable()
    }

    /**
     * For when nobody has the recovery key any more: deletes the backup in your Google account
     * (it can't be opened without its key), makes a new key, and uploads what's on this phone as
     * the new backup. Other devices then ask for the new recovery key.
     */
    suspend fun startOver() = guarded("start over") {
        val token = pendingToken ?: platform.silentToken() ?: throw IllegalStateException("Sign in to Google again first")
        val drive = DriveClient(token)
        // Drive can hold more than one file with the same name; remove them all.
        var tries = 0
        while (tries++ < 20) {
            val old = drive.find(FILE) ?: break
            drive.delete(old.id)
        }
        vault.save(Crypto.newKey())
        pushNow(drive)
        enable()
    }

    suspend fun turnOff(forgetKey: Boolean) {
        pushJob?.cancel()
        prefs.clear()
        platform.signOut()
        if (forgetKey) vault.forget()
        pendingToken = null
        _state.value = State()
    }

    fun recoveryKey(): String? = vault.local()?.let(Crypto::keyToText)

    // --- Staying in sync -------------------------------------------------------------------

    /** Something changed locally: upload shortly, batching quick edits together. */
    fun onLocalChange() {
        if (!_state.value.enabled || System.currentTimeMillis() < ignoreChangesUntil) return
        prefs.putBoolean(DIRTY, true)
        pushJob?.cancel()
        pushJob = scope.launch {
            delay(4_000)
            sync()
        }
    }

    /** Opening the app: bring in anything newer. Leaving it: send anything pending. */
    fun onForeground() {
        val now = System.currentTimeMillis()
        if (!_state.value.enabled || now - lastForeground < 30_000) return
        lastForeground = now
        scope.launch { sync() }
    }

    /** Sends anything waiting right now; desktop calls this before the window closes. */
    suspend fun flush() {
        pushJob?.cancel()
        if (_state.value.enabled && prefs.getBoolean(DIRTY, false)) sync()
    }
    fun onBackground() { if (_state.value.enabled && prefs.getBoolean(DIRTY, false)) { pushJob?.cancel(); scope.launch { sync() } } }

    fun syncNow() = scope.launch { sync() }

    /** Runs sync work in the app's scope, so it carries on if the screen that started it closes. */
    fun launch(block: suspend SyncManager.() -> Unit) = scope.launch { block() }

    private suspend fun sync() = guarded("sync") {
        val token = platform.silentToken()
            ?: run { _state.update { it.copy(problem = Problem.NeedsSignIn) }; return@guarded }
        val key = vault.local() ?: run { _state.update { it.copy(problem = Problem.NeedsRecoveryKey) }; return@guarded }
        try {
            syncWith(DriveClient(token), key)
        } catch (e: DriveClient.Unauthorized) {
            platform.forgetToken(token)
            val again = platform.silentToken()
                ?: run { _state.update { it.copy(problem = Problem.NeedsSignIn) }; return@guarded }
            syncWith(DriveClient(again), key)
        }
    }

    private suspend fun syncWith(drive: DriveClient, key: ByteArray) {
        val remote = drive.find(FILE)
        val dirty = prefs.getBoolean(DIRTY, false)
        val changedElsewhere = remote != null && remote.modifiedTime != prefs.getString(REMOTE_VERSION)
        if (dirty && changedElsewhere) {
            // Only overwrite another device's upload if it uses our key: after a "start over"
            // elsewhere it won't, and replacing it would undo that. Throws WrongKey otherwise.
            Crypto.open(key, drive.download(remote!!.id))
        }
        when {
            dirty || remote == null -> pushNow(drive, remote)
            changedElsewhere -> restoreFrom(drive, key, remote)
            else -> markSynced(remote.modifiedTime)
        }
    }

    // --- Moving bytes -----------------------------------------------------------------------

    private suspend fun pushNow(drive: DriveClient, known: DriveClient.RemoteFile? = null) {
        val key = vault.local() ?: throw IllegalStateException("No key")
        val blob = Crypto.seal(key, Backup.export(db))
        val existing = known ?: drive.find(FILE)
        val saved = if (existing != null) drive.update(existing.id, blob) else drive.create(FILE, blob)
        prefs.putBoolean(DIRTY, false)
        markSynced(saved.modifiedTime)
    }

    private suspend fun restoreFrom(drive: DriveClient, key: ByteArray, known: DriveClient.RemoteFile? = null) {
        val remote = known ?: drive.find(FILE) ?: return
        val json = Crypto.open(key, drive.download(remote.id))
        ignoreChangesUntil = System.currentTimeMillis() + 3_000
        Backup.import(db, json)
        prefs.putBoolean(DIRTY, false)
        markSynced(remote.modifiedTime)
    }

    private fun markSynced(remoteVersion: String?) {
        val now = System.currentTimeMillis()
        prefs.putString(REMOTE_VERSION, remoteVersion); prefs.putLong(LAST_SYNC, now)
        _state.update { it.copy(lastSyncedAt = now, problem = null) }
    }

    private suspend fun enable() {
        prefs.putBoolean(ENABLED, true)
        pendingToken = null
        _state.update { it.copy(enabled = true, problem = null, keyFollows = vault.followsToNewDevice()) }
    }

    private suspend fun guarded(what: String, block: suspend () -> Unit) = mutex.withLock {
        _state.update { it.copy(busy = true) }
        try {
            block()
        } catch (e: Crypto.WrongKey) {
            _state.update { it.copy(problem = Problem.NeedsRecoveryKey) }
        } catch (e: kotlinx.coroutines.CancellationException) {
            throw e
        } catch (e: Exception) {
            _state.update { it.copy(problem = Problem.Failed(e.message ?: "Couldn't $what")) }
        } finally {
            _state.update { it.copy(busy = false) }
        }
    }

    private companion object {
        const val FILE = "memi-backup.bin"
        const val ENABLED = "enabled"
        const val ACCOUNT = "account"
        const val DIRTY = "dirty"
        const val LAST_SYNC = "lastSync"
        const val REMOTE_VERSION = "remoteVersion"
    }
}
