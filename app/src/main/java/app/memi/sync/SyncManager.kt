package app.memi.sync

import app.memi.data.AppDatabase
import app.memi.data.Backup
import app.memi.data.Merge
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/**
 * Keeps an encrypted copy of everything in the Google account you pick, and every device that
 * shares it in step. Changes go up a couple of seconds after you make them; while Memi is open it
 * checks for changes from your other devices every few seconds.
 *
 * Both copies are merged item by item (data/Merge.kt), so edits on two devices at once are both
 * kept; only the same item edited in two places at once keeps the later edit.
 */
class SyncManager(private val platform: SyncPlatform, private val db: AppDatabase, private val scope: CoroutineScope) {

    sealed interface Problem {
        /** Google wants you to sign in again (access was removed, or the account changed). */
        data object NeedsSignIn : Problem
        /**
         * There's a backup but this phone doesn't have its key. [confirm] is the code shown while
         * this phone is asking your other devices for it.
         */
        data class NeedsRecoveryKey(val confirm: String? = null) : Problem
        data class Failed(val message: String) : Problem
    }

    data class State(
        val enabled: Boolean = false,
        val account: String? = null,
        val busy: Boolean = false,
        val lastSyncedAt: Long? = null,
        val problem: Problem? = null,
        val keyFollows: Boolean = false,
        /** Other devices asking for the key, to allow or turn down. */
        val joins: List<Join> = emptyList(),
    )

    /** Another device asking for the key. */
    data class Join(val fileId: String, val device: String, val confirm: String)

    private val prefs = platform.settings
    private val vault = platform.keys
    private val mutex = Mutex()
    private var pushJob: Job? = null
    private var liveJob: Job? = null
    private var pairingJob: Job? = null
    private var offer: Pairing.Offer? = null
    private var ignoreChangesUntil = 0L
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
            key == null -> askForKey(drive)
            else -> {
                vault.save(key)
                // Throws WrongKey (and asks for the key) if the backup was made with another one.
                mergeWith(drive, key, remote)
                enable()
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
            stopAsking()
            vault.save(key)
            _state.update { it.copy(problem = null) }
            connect(token)
        }
        return ok
    }

    // --- Getting the key from another device ("start anywhere") --------------------------------

    /**
     * Leaves a join request in Drive and waits for one of your other devices to answer it. The
     * person compares the code this shows with the one on the other device and taps Allow there.
     */
    private suspend fun askForKey(drive: DriveClient) {
        stopAsking()
        val o = Pairing.Offer()
        offer = o
        runCatching { drive.create(o.joinFileName, o.joinRequest(platform.deviceLabel)) }
        _state.update { it.copy(problem = Problem.NeedsRecoveryKey(o.confirmCode)) }
        pairingJob = scope.launch {
            val until = System.currentTimeMillis() + Pairing.JOIN_MINUTES * 60_000
            while (isActive && System.currentTimeMillis() < until) {
                delay(3_000)
                if (runCatching { checkPairing(o) }.getOrDefault(false)) return@launch
            }
            // The request has expired; still let the person type the recovery key.
            if (offer === o) _state.update { it.copy(problem = Problem.NeedsRecoveryKey(null)) }
        }
    }

    /** Looks once for another device's answer. True once the key is in. */
    private suspend fun checkPairing(o: Pairing.Offer): Boolean {
        val token = pendingToken ?: platform.silentToken() ?: return false
        val drive = DriveClient(token)
        val file = drive.find(o.fileName) ?: return false
        val blob = drive.download(file.id)
        runCatching { drive.delete(file.id) }
        val key = o.open(blob)
        // This runs in pairingJob; finishing up mustn't cancel it.
        pairingJob = null
        return adoptKey(key)
    }

    private fun stopAsking() {
        pairingJob?.cancel()
        pairingJob = null
        val o = offer ?: return
        offer = null
        scope.launch {
            runCatching {
                val drive = DriveClient(pendingToken ?: platform.silentToken() ?: return@launch)
                drive.find(o.joinFileName)?.let { drive.delete(it.id) }
            }
        }
    }

    // --- Giving the key to another device -------------------------------------------------------

    /** Finds devices asking for the key; old or broken requests are cleared away. */
    private suspend fun lookForJoins(drive: DriveClient) {
        val found = mutableListOf<Join>()
        for (f in drive.list(Pairing.JOIN_PREFIX)) {
            if (f.name == offer?.joinFileName) continue
            val req = runCatching { Pairing.parseJoinRequest(drive.download(f.id)) }.getOrNull()
            if (req == null || System.currentTimeMillis() - req.at > Pairing.JOIN_MINUTES * 60_000) {
                runCatching { drive.delete(f.id) }
                continue
            }
            found += Join(f.id, req.device, req.scanned.confirmCode)
        }
        _state.update { it.copy(joins = found) }
    }

    /** Allow: sends this device's key to the one that asked, so only it can open it. */
    suspend fun approveJoin(fileId: String): String? {
        val key = vault.local() ?: return "Turn on sync here first."
        val token = platform.silentToken() ?: return "Sign in to Google again in Sync first."
        return runCatching {
            val drive = DriveClient(token)
            val req = Pairing.parseJoinRequest(drive.download(fileId)) ?: return "That request has expired."
            drive.create(req.scanned.fileName, Pairing.sealFor(req.scanned, key))
            runCatching { drive.delete(fileId) }
            _state.update { s -> s.copy(joins = s.joins.filter { it.fileId != fileId }) }
            null
        }.getOrElse { "Couldn't reach Google Drive: ${it.message}" }
    }

    /** Not mine: removes the request. */
    suspend fun declineJoin(fileId: String) {
        _state.update { s -> s.copy(joins = s.joins.filter { it.fileId != fileId }) }
        runCatching { DriveClient(platform.silentToken() ?: return).delete(fileId) }
    }

    // --- Giving a computer the key by QR code (see Pairing.kt) --------------------------------

    /** On the phone: sends this phone's key to the computer whose code was scanned. */
    suspend fun sendKeyTo(code: String): String {
        val scanned = Pairing.parse(code) ?: return "That isn't a Memi code. On the other device, open Sync to show one."
        val key = vault.local() ?: return "Turn on sync on this phone first."
        val token = platform.silentToken() ?: return "Sign in to Google again in Sync first."
        return runCatching {
            DriveClient(token).create(scanned.fileName, Pairing.sealFor(scanned, key))
            "Sent. The other device will finish setting up in a few seconds."
        }.getOrElse { "Couldn't reach Google Drive: ${it.message}" }
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
        stopAsking()
        vault.save(Crypto.newKey())
        pushNow(drive)
        enable()
    }

    suspend fun turnOff(forgetKey: Boolean) {
        pushJob?.cancel()
        liveJob?.cancel()
        stopAsking()
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
            delay(PUSH_DELAY)
            sync()
        }
    }

    /** While Memi is open: check for changes from your other devices every few seconds. */
    fun onForeground() {
        liveJob?.cancel()
        liveJob = scope.launch {
            while (isActive) {
                if (_state.value.enabled) sync()
                delay(LIVE_SECONDS * 1_000)
            }
        }
    }

    /** Leaving Memi: stop checking, and send anything waiting. */
    fun onBackground() {
        liveJob?.cancel()
        liveJob = null
        if (_state.value.enabled && prefs.getBoolean(DIRTY, false)) { pushJob?.cancel(); scope.launch { sync() } }
    }

    fun syncNow() = scope.launch { sync() }

    /** Runs sync work in the app's scope, so it carries on if the screen that started it closes. */
    fun launch(block: suspend SyncManager.() -> Unit) = scope.launch { block() }

    private suspend fun sync() = guarded("sync") {
        val token = platform.silentToken()
            ?: run { _state.update { it.copy(problem = Problem.NeedsSignIn) }; return@guarded }
        val key = vault.local() ?: run { _state.update { it.copy(problem = Problem.NeedsRecoveryKey()) }; return@guarded }
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
        when {
            remote == null -> pushNow(drive)
            remote.modifiedTime != prefs.getString(REMOTE_VERSION) -> mergeWith(drive, key, remote)
            prefs.getBoolean(DIRTY, false) -> pushNow(drive, remote)
            else -> markSynced(remote.modifiedTime)
        }
        runCatching { lookForJoins(drive) }
    }

    // --- Moving bytes -----------------------------------------------------------------------

    private suspend fun pushNow(drive: DriveClient, known: DriveClient.RemoteFile? = null) {
        val key = vault.local() ?: throw IllegalStateException("No key")
        val blob = Crypto.seal(key, Backup.export(Backup.snapshot(db)))
        val existing = known ?: drive.find(FILE)
        val saved = if (existing != null) drive.update(existing.id, blob) else drive.create(FILE, blob)
        prefs.putBoolean(DIRTY, false)
        markSynced(saved.modifiedTime)
    }

    /**
     * Downloads the copy in Drive, merges it with this phone's, and saves the result on whichever
     * side needs it. A copy made with another key throws WrongKey and nothing changes.
     */
    private suspend fun mergeWith(drive: DriveClient, key: ByteArray, remote: DriveClient.RemoteFile) {
        val doc = Backup.import(Crypto.open(key, drive.download(remote.id)))
        val merged = Merge.merge(Backup.snapshot(db), doc.snapshot)
        if (merged.localChanged) {
            ignoreChangesUntil = System.currentTimeMillis() + 2_000
            Backup.apply(db, merged.snapshot, merged.peopleFromRemote, doc.photos)
        }
        if (merged.remoteChanged || doc.legacy) pushNow(drive, remote)
        else {
            prefs.putBoolean(DIRTY, false)
            markSynced(remote.modifiedTime)
        }
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
            if (_state.value.problem !is Problem.NeedsRecoveryKey) {
                val drive = runCatching { DriveClient(pendingToken ?: platform.silentToken()!!) }.getOrNull()
                if (drive != null) runCatching { askForKey(drive) } else _state.update { it.copy(problem = Problem.NeedsRecoveryKey()) }
            }
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
        const val PUSH_DELAY = 2_000L
        const val LIVE_SECONDS = 10L
    }
}
