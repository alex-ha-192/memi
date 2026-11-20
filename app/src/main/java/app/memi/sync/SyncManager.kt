package app.memi.sync

import app.memi.data.Repository
import app.memi.engine.Engine
import app.memi.engine.Host
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import app.memi.engine.Problem as EngineProblem

/**
 * Keeps an encrypted copy of everything in the Google account you pick, and every device that
 * shares it in step. Changes go up a couple of seconds after you make them; while Memi is open it
 * checks for changes from your other devices every few seconds.
 *
 * The work itself (merging item by item, encryption, asking other devices for the key, starting
 * over) is memi-engine's, the same on every device. This class gives it Google access tokens from
 * Play services, runs it off the main thread on a timer, and turns its status into [State] for
 * the Sync sheet.
 */
class SyncManager(
    private val platform: SyncPlatform,
    private val engine: Engine,
    private val repo: Repository,
    private val scope: CoroutineScope,
) {

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

    private val mutex = Mutex()
    private var pushJob: Job? = null
    private var liveJob: Job? = null
    private var pairingJob: Job? = null
    private var pairingId: String? = null
    /** Shown instead of the engine's problem when there's no Google token at all. */
    private var signInNeeded = false

    private val _state = MutableStateFlow(read(keyFollows = false))
    val state: StateFlow<State> = _state.asStateFlow()

    init {
        if (_state.value.enabled) scope.launch { _state.update { it.copy(keyFollows = platform.keys.followsToNewDevice()) } }
    }

    /** The engine's status, as the Sync sheet shows it. */
    private fun read(keyFollows: Boolean): State {
        val s = engine.status()
        val problem = when {
            signInNeeded -> Problem.NeedsSignIn
            else -> when (val p = s.problem) {
                is EngineProblem.NeedsSignIn -> Problem.NeedsSignIn
                is EngineProblem.NeedsKey -> Problem.NeedsRecoveryKey(s.asking?.confirm)
                is EngineProblem.Failed -> Problem.Failed(p.message)
                null -> s.asking?.let { Problem.NeedsRecoveryKey(it.confirm) }
            }
        }
        return State(
            enabled = s.sync.enabled,
            account = s.sync.account,
            busy = s.busy,
            lastSyncedAt = s.sync.lastSync,
            problem = problem,
            keyFollows = keyFollows,
            joins = s.joins.map { Join(it.fileId, it.device, it.confirm) },
        )
    }

    /** Shows the engine's latest status and data, and waits for another device's answer if it's asking. */
    private fun publish() {
        _state.value = read(_state.value.keyFollows)
        repo.refresh()
        val asking = engine.status().asking
        if (asking != null && asking.id != pairingId) waitForKey(asking.id)
    }

    /**
     * Runs one engine step with a Google token (Play services gives one without asking). If Google
     * turns it down, drops it and tries once more with a fresh one. False if there's no token.
     */
    private suspend fun withToken(first: String? = null, step: Engine.(String) -> Unit): Boolean = withContext(Dispatchers.IO) {
        _state.update { it.copy(busy = true) }
        try {
            var token = first ?: platform.silentToken()
            if (token == null) { signInNeeded = true; return@withContext false }
            signInNeeded = false
            engine.step(token)
            if (engine.needsSignIn()) {
                platform.forgetToken(token)
                token = platform.silentToken() ?: run { signInNeeded = true; return@withContext false }
                engine.clearProblem()
                engine.step(token)
            }
            true
        } finally {
            publish()
        }
    }

    // --- Turning it on ------------------------------------------------------------------------

    /** Called with a token right after you've picked your Google account. */
    suspend fun connect(token: String) = mutex.withLock {
        // A key this phone backed up with Block Store comes back on a new phone.
        if (platform.keys.local() == null) platform.keys.fromCloud()?.let { platform.keys.save(it) }
        withToken(token) { connect(it) }
        if (_state.value.enabled) _state.update { it.copy(keyFollows = platform.keys.followsToNewDevice()) }
    }

    /** True if [text] opens the backup in your account; sync is then on. */
    suspend fun useRecoveryKey(text: String): Boolean {
        var ok = false
        mutex.withLock { withToken { ok = useRecoveryKey(it, text) } }
        if (ok) { stopWaiting(); _state.update { it.copy(keyFollows = platform.keys.followsToNewDevice()) } }
        return ok
    }

    // --- Getting the key from another device ("start anywhere") --------------------------------

    /** The engine left a request for the key in Drive: check for an answer every few seconds. */
    private fun waitForKey(id: String) {
        pairingJob?.cancel()
        pairingId = id
        pairingJob = scope.launch(Dispatchers.IO) {
            val until = System.currentTimeMillis() + JOIN_MINUTES * 60_000
            while (isActive && System.currentTimeMillis() < until) {
                delay(3_000)
                if (engine.status().asking?.id != id) return@launch
                val token = platform.silentToken() ?: continue
                if (runCatching { engine.checkPairing(token) }.getOrDefault(false)) {
                    pairingId = null
                    _state.update { it.copy(keyFollows = platform.keys.followsToNewDevice()) }
                    publish()
                    return@launch
                }
            }
            // The request has expired; the person can still type the recovery key.
            if (isActive) { engine.cancelAsking(platform.silentToken()); publish() }
        }
    }

    private fun stopWaiting() {
        pairingJob?.cancel()
        pairingJob = null
        pairingId = null
    }

    // --- Giving the key to another device -------------------------------------------------------

    /** Allow: sends this device's key to the one that asked, so only it can open it. */
    suspend fun approveJoin(fileId: String): String? = withContext(Dispatchers.IO) {
        if (engine.recoveryKey() == null) return@withContext "Turn on sync here first."
        val token = platform.silentToken() ?: return@withContext "Sign in to Google again in Sync first."
        val result = runCatching { engine.answerJoin(token, fileId, true) }
        publish()
        result.exceptionOrNull()?.let { "Couldn't reach Google Drive: ${it.message}" }
    }

    /** Not mine: removes the request. */
    suspend fun declineJoin(fileId: String) = withContext(Dispatchers.IO) {
        val token = platform.silentToken() ?: return@withContext
        runCatching { engine.answerJoin(token, fileId, false) }
        publish()
    }

    /** On the phone: sends this phone's key to the computer whose QR code was scanned. */
    suspend fun sendKeyTo(code: String): String = withContext(Dispatchers.IO) {
        val token = platform.silentToken() ?: return@withContext "Sign in to Google again in Sync first."
        runCatching { engine.sendKeyTo(token, code) }.fold(
            onSuccess = { "Sent. The other device will finish setting up in a few seconds." },
            onFailure = { it.message ?: "Couldn't reach Google Drive." },
        )
    }

    /**
     * For when nobody has the recovery key any more: replaces the backup in your Google account
     * with what's on this phone, under a new key. Other devices then ask for the new key.
     */
    suspend fun startOver() = mutex.withLock {
        stopWaiting()
        withToken { startOver(it) }
        _state.update { it.copy(keyFollows = platform.keys.followsToNewDevice()) }
    }

    suspend fun turnOff(forgetKey: Boolean) = withContext(Dispatchers.IO) {
        pushJob?.cancel()
        liveJob?.cancel()
        stopWaiting()
        engine.cancelAsking(platform.silentToken())
        engine.turnOff(forgetKey)
        platform.settings.clear()
        platform.signOut()
        signInNeeded = false
        _state.value = read(keyFollows = false)
    }

    fun recoveryKey(): String? = engine.recoveryKey()

    // --- Staying in sync -------------------------------------------------------------------

    /** You changed something: upload shortly, batching quick edits together. */
    fun onLocalChange() {
        if (!_state.value.enabled) return
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
        if (_state.value.enabled && engine.status().sync.dirty) { pushJob?.cancel(); scope.launch { sync() } }
    }

    fun syncNow() = scope.launch { sync() }

    /** Runs sync work in the app's scope, so it carries on if the screen that started it closes. */
    fun launch(block: suspend SyncManager.() -> Unit) = scope.launch { block() }

    private suspend fun sync() = mutex.withLock {
        if (engine.status().sync.enabled) withToken { syncNow(it) }
    }

    private companion object {
        const val PUSH_DELAY = 2_000L
        const val LIVE_SECONDS = 10L
        const val JOIN_MINUTES = 15L
    }
}

/**
 * What the engine needs from Android: the key, kept in the Keystore and backed up with Block
 * Store ([KeyVault]), and this phone's name. The engine calls these from a background thread.
 */
class EngineHost(private val platform: SyncPlatform) : Host {
    override fun key(): ByteArray? = platform.keys.local()
    override fun saveKey(key: ByteArray) = runBlocking { platform.keys.save(key) }
    override fun forgetKey() = runBlocking { platform.keys.forget() }
    override fun deviceLabel(): String = platform.deviceLabel
}
