package app.memi.sync

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import com.google.android.gms.auth.blockstore.Blockstore
import com.google.android.gms.auth.blockstore.DeleteBytesRequest
import com.google.android.gms.auth.blockstore.RetrieveBytesRequest
import com.google.android.gms.auth.blockstore.StoreBytesData
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Sync on Android: Google Play services sign-in, Keystore for the key, Block Store to carry it to a new phone. */
class AndroidSyncPlatform(private val context: Context) : SyncPlatform {
    override val settings: SettingsStore = PrefsStore(context)
    override val keys: KeyVault = AndroidKeyVault(context)
    override val deviceLabel: String = android.os.Build.MODEL?.takeIf { it.isNotBlank() } ?: "Android phone"

    override suspend fun silentToken(): String? = when (val o = runCatching { GoogleAuth.authorize(context) }.getOrNull()) {
        is GoogleAuth.Outcome.Token -> o.value
        else -> null
    }

    override suspend fun forgetToken(token: String) = GoogleAuth.forget(context, token)

    override suspend fun signOut() = Unit
}

private class PrefsStore(context: Context) : SettingsStore {
    private val prefs = context.getSharedPreferences("sync", Context.MODE_PRIVATE)
    override fun getBoolean(key: String, default: Boolean) = prefs.getBoolean(key, default)
    override fun getString(key: String): String? = prefs.getString(key, null)
    override fun getLong(key: String, default: Long) = prefs.getLong(key, default)
    override fun putBoolean(key: String, value: Boolean) = prefs.edit().putBoolean(key, value).apply()
    override fun putString(key: String, value: String?) = prefs.edit().putString(key, value).apply()
    override fun putLong(key: String, value: Long) = prefs.edit().putLong(key, value).apply()
    override fun clear() = prefs.edit().clear().apply()
}

/**
 * Where the sync key lives on Android.
 * - On this phone: wrapped by a hardware-backed Android Keystore key that never leaves the device.
 * - For your next phone: Google's Block Store, which backs it up end-to-end encrypted with your
 *   screen lock and hands it back when you set up a new phone from your Google account.
 */
private class AndroidKeyVault(private val context: Context) : KeyVault {
    private val prefs = context.getSharedPreferences("sync_key", Context.MODE_PRIVATE)

    override fun local(): ByteArray? {
        val iv = prefs.getString("iv", null) ?: return null
        val wrapped = prefs.getString("key", null) ?: return null
        return runCatching {
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, wrappingKey(), GCMParameterSpec(128, Base64.decode(iv, Base64.NO_WRAP)))
            cipher.doFinal(Base64.decode(wrapped, Base64.NO_WRAP))
        }.getOrNull()
    }

    override suspend fun fromCloud(): ByteArray? = runCatching {
        val res = Blockstore.getClient(context)
            .retrieveBytes(RetrieveBytesRequest.Builder().setKeys(listOf(BLOCK_KEY)).build())
            .await()
        res.blockstoreDataMap[BLOCK_KEY]?.bytes
    }.getOrNull()

    override suspend fun save(key: ByteArray) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, wrappingKey())
        prefs.edit()
            .putString("iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
            .putString("key", Base64.encodeToString(cipher.doFinal(key), Base64.NO_WRAP))
            .apply()
        runCatching {
            Blockstore.getClient(context).storeBytes(
                StoreBytesData.Builder().setKey(BLOCK_KEY).setBytes(key).setShouldBackupToCloud(true).build()
            ).await()
        }
    }

    override suspend fun followsToNewDevice(): Boolean =
        runCatching { Blockstore.getClient(context).isEndToEndEncryptionAvailable().await() }.getOrDefault(false)

    override suspend fun forget() {
        prefs.edit().clear().apply()
        runCatching { KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.deleteEntry(WRAP_ALIAS) }
        runCatching {
            Blockstore.getClient(context).deleteBytes(DeleteBytesRequest.Builder().setKeys(listOf(BLOCK_KEY)).build()).await()
        }
    }

    private fun wrappingKey(): SecretKey {
        val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (ks.getEntry(WRAP_ALIAS, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        gen.init(
            KeyGenParameterSpec.Builder(WRAP_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build()
        )
        return gen.generateKey()
    }

    private companion object {
        const val BLOCK_KEY = "memi_sync_key"
        const val WRAP_ALIAS = "memi_sync_wrap"
    }
}
