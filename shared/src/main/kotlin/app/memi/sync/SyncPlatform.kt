package app.memi.sync

/**
 * What the shared sync engine needs from each app. Android uses Google Play services and the
 * Android Keystore; desktop signs in through the browser and uses the system password store.
 */
interface SyncPlatform {
    /** Small settings like "sync is on" and when it last ran. */
    val settings: SettingsStore

    /** Where the encryption key lives on this device. */
    val keys: KeyVault

    /** An access token for the connected Google account without asking, or null if you need to sign in again. */
    suspend fun silentToken(): String?

    /** Drops a token Google rejected, so the next [silentToken] fetches a fresh one. */
    suspend fun forgetToken(token: String)

    /** Called when sync is turned off: forget the account on this device. */
    suspend fun signOut()

    /** "phone" or "computer", for the words in settings. */
    val deviceWord: String
}

interface SettingsStore {
    fun getBoolean(key: String, default: Boolean = false): Boolean
    fun getString(key: String): String?
    fun getLong(key: String, default: Long = 0L): Long
    fun putBoolean(key: String, value: Boolean)
    fun putString(key: String, value: String?)
    fun putLong(key: String, value: Long)
    fun clear()
}

interface KeyVault {
    /** The key kept on this device, if there is one. */
    fun local(): ByteArray?

    /** The key backed up from another device (Android's Block Store); null where that doesn't exist. */
    suspend fun fromCloud(): ByteArray?

    /** Keeps the key on this device (and backs it up where the platform can). */
    suspend fun save(key: ByteArray)

    /** True if the key will follow you to a new device without the recovery key. */
    suspend fun followsToNewDevice(): Boolean

    suspend fun forget()
}
