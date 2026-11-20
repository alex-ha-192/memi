package app.memi.sync

import app.memi.platform.AppDirs
import com.github.javakeyring.Keyring
import com.sun.net.httpserver.HttpServer
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import org.json.JSONObject
import java.awt.Desktop
import java.io.File
import java.net.HttpURLConnection
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.URI
import java.net.URL
import java.net.URLDecoder
import java.net.URLEncoder
import java.nio.file.Files
import java.nio.file.attribute.PosixFilePermissions
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.Base64
import java.util.Properties

/** Sync on desktop: browser sign-in, and the system password store for secrets. */
class DesktopSyncPlatform : SyncPlatform {
    override val settings: SettingsStore = PropertiesStore(File(AppDirs.data, "sync.properties"))
    override val keys: KeyVault = DesktopKeyVault
    override val deviceWord = "computer"

    override suspend fun silentToken(): String? = DesktopGoogleAuth.silentToken()
    override suspend fun forgetToken(token: String) = DesktopGoogleAuth.forget(token)
    override suspend fun signOut() = DesktopGoogleAuth.signOut()
}

/** Small settings in a properties file next to the database. */
private class PropertiesStore(private val file: File) : SettingsStore {
    private val props = Properties().apply { if (file.exists()) file.inputStream().use { load(it) } }

    @Synchronized private fun save() { file.outputStream().use { props.store(it, "Memi sync") } }

    override fun getBoolean(key: String, default: Boolean) = props.getProperty(key)?.toBooleanStrictOrNull() ?: default
    override fun getString(key: String): String? = props.getProperty(key)
    override fun getLong(key: String, default: Long) = props.getProperty(key)?.toLongOrNull() ?: default
    @Synchronized override fun putBoolean(key: String, value: Boolean) { props.setProperty(key, value.toString()); save() }
    @Synchronized override fun putString(key: String, value: String?) { if (value == null) props.remove(key) else props.setProperty(key, value); save() }
    @Synchronized override fun putLong(key: String, value: Long) { props.setProperty(key, value.toString()); save() }
    @Synchronized override fun clear() { props.clear(); save() }
}

/** The sync key on this computer, kept in the system password store. There's no cloud copy: other devices use the recovery key. */
private object DesktopKeyVault : KeyVault {
    private const val NAME = "sync-key"
    override fun local(): ByteArray? = DesktopSecrets.get(NAME)?.let { runCatching { Base64.getDecoder().decode(it) }.getOrNull() }
    override suspend fun fromCloud(): ByteArray? = null
    override suspend fun save(key: ByteArray) = DesktopSecrets.set(NAME, Base64.getEncoder().encodeToString(key))
    override suspend fun followsToNewDevice() = false
    override suspend fun forget() = DesktopSecrets.delete(NAME)
}

/**
 * Secrets (the sync key and Google's sign-in token) go in Windows Credential Manager or the
 * Linux keyring (GNOME Keyring / KWallet). If there's no keyring, they fall back to a file in
 * Memi's folder that only your user account can read.
 */
object DesktopSecrets {
    private const val SERVICE = "Memi"
    private val file = File(AppDirs.data, "secrets.properties")

    @Synchronized fun get(name: String): String? =
        runCatching { Keyring.create().use { it.getPassword(SERVICE, name) } }.getOrNull() ?: fileProps().getProperty(name)

    @Synchronized fun set(name: String, value: String) {
        val stored = runCatching { Keyring.create().use { it.setPassword(SERVICE, name, value) } }.isSuccess
        val props = fileProps()
        if (stored) props.remove(name) else props.setProperty(name, value)
        writeFile(props)
    }

    @Synchronized fun delete(name: String) {
        runCatching { Keyring.create().use { it.deletePassword(SERVICE, name) } }
        writeFile(fileProps().apply { remove(name) })
    }

    private fun fileProps() = Properties().apply { if (file.exists()) file.inputStream().use { load(it) } }

    private fun writeFile(props: Properties) {
        if (props.isEmpty) { file.delete(); return }
        if (!file.exists()) {
            file.createNewFile()
            runCatching { Files.setPosixFilePermissions(file.toPath(), PosixFilePermissions.fromString("rw-------")) }
        }
        file.outputStream().use { props.store(it, null) }
    }
}

/**
 * Google sign-in for desktop apps: opens your browser to Google's page, where you pick the
 * account, and Google sends you back to a one-off address on this computer (127.0.0.1). Uses
 * PKCE, and only asks for Memi's hidden app-data folder in Drive.
 *
 * Needs the "Desktop app" OAuth client from your Google Cloud project, saved as
 * desktop/src/main/resources/google-client.json (see the README).
 */
object DesktopGoogleAuth {
    private const val SCOPE = "https://www.googleapis.com/auth/drive.appdata"
    private const val AUTH = "https://accounts.google.com/o/oauth2/v2/auth"
    private const val TOKEN = "https://oauth2.googleapis.com/token"
    private const val REFRESH = "google-refresh-token"

    private data class Client(val id: String, val secret: String)

    private val client: Client? by lazy {
        runCatching {
            val text = DesktopGoogleAuth::class.java.getResource("/google-client.json")!!.readText()
            val root = JSONObject(text)
            val o = root.optJSONObject("installed") ?: root.optJSONObject("web") ?: root
            Client(o.getString("client_id"), o.getString("client_secret"))
        }.getOrNull()
    }

    @Volatile private var access: String? = null
    @Volatile private var expiresAt = 0L

    suspend fun signIn(): String = withContext(Dispatchers.IO) {
        val c = client ?: error("Sync isn't set up in this build yet. See \"Desktop sync\" in the README.")
        val random = SecureRandom()
        fun token(n: Int) = Base64.getUrlEncoder().withoutPadding().encodeToString(ByteArray(n).also(random::nextBytes))
        val verifier = token(48)
        val challenge = Base64.getUrlEncoder().withoutPadding()
            .encodeToString(MessageDigest.getInstance("SHA-256").digest(verifier.toByteArray(Charsets.US_ASCII)))
        val state = token(16)

        val reply = CompletableDeferred<Map<String, String>>()
        val server = HttpServer.create(InetSocketAddress(InetAddress.getByName("127.0.0.1"), 0), 0)
        server.createContext("/") { ex ->
            val q = parseQuery(ex.requestURI.rawQuery)
            val ok = q["code"] != null
            val page = """<!doctype html><meta charset="utf-8"><title>Memi</title>
                <body style="font-family:system-ui,sans-serif;background:#F4F3F8;color:#1C1B2E;display:grid;place-items:center;height:90vh">
                <div style="text-align:center"><h2>${if (ok) "You're signed in" else "Sign-in didn't finish"}</h2>
                <p>You can close this tab and go back to Memi.</p></div>""".trimIndent().toByteArray()
            ex.responseHeaders.add("Content-Type", "text/html; charset=utf-8")
            ex.sendResponseHeaders(200, page.size.toLong())
            ex.responseBody.use { it.write(page) }
            if (q.containsKey("code") || q.containsKey("error")) reply.complete(q)
        }
        server.start()
        try {
            val redirect = "http://127.0.0.1:${server.address.port}"
            val url = AUTH + "?" + form(
                "client_id" to c.id, "redirect_uri" to redirect, "response_type" to "code", "scope" to SCOPE,
                "code_challenge" to challenge, "code_challenge_method" to "S256", "state" to state,
                "access_type" to "offline", "prompt" to "select_account consent",
            )
            openBrowser(url)
            val q = withTimeout(5 * 60_000) { reply.await() }
            if (q["error"] != null) error("Sign-in was cancelled.")
            check(q["state"] == state) { "Sign-in reply didn't match. Please try again." }
            val res = post(
                "code" to q.getValue("code"), "client_id" to c.id, "client_secret" to c.secret,
                "redirect_uri" to redirect, "grant_type" to "authorization_code", "code_verifier" to verifier,
            )
            res.optString("refresh_token").takeIf { it.isNotEmpty() }?.let { DesktopSecrets.set(REFRESH, it) }
            remember(res)
        } finally {
            server.stop(0)
        }
    }

    /** A fresh access token from the saved sign-in, or null if you need to sign in again. */
    suspend fun silentToken(): String? = withContext(Dispatchers.IO) {
        access?.takeIf { System.currentTimeMillis() < expiresAt - 60_000 }?.let { return@withContext it }
        val c = client ?: return@withContext null
        val refresh = DesktopSecrets.get(REFRESH) ?: return@withContext null
        try {
            remember(post("client_id" to c.id, "client_secret" to c.secret, "refresh_token" to refresh, "grant_type" to "refresh_token"))
        } catch (e: TokenRejected) {
            DesktopSecrets.delete(REFRESH)
            null
        }
    }

    fun forget(token: String) {
        if (access == token) access = null
    }

    fun signOut() {
        access = null
        DesktopSecrets.delete(REFRESH)
    }

    private class TokenRejected : Exception("Google needs you to sign in again")

    private fun remember(res: JSONObject): String {
        val token = res.getString("access_token")
        access = token
        expiresAt = System.currentTimeMillis() + res.optLong("expires_in", 3600) * 1000
        return token
    }

    private fun post(vararg fields: Pair<String, String>): JSONObject {
        val body = form(*fields).toByteArray()
        val c = URL(TOKEN).openConnection() as HttpURLConnection
        try {
            c.requestMethod = "POST"
            c.doOutput = true
            c.connectTimeout = 20_000
            c.readTimeout = 30_000
            c.setRequestProperty("Content-Type", "application/x-www-form-urlencoded")
            c.outputStream.use { it.write(body) }
            val code = c.responseCode
            val text = (if (code in 200..299) c.inputStream else c.errorStream)?.use { String(it.readBytes()) }.orEmpty()
            if (code == 400 && text.contains("invalid_grant")) throw TokenRejected()
            if (code !in 200..299) error("Google sign-in failed ($code)")
            return JSONObject(text)
        } finally {
            c.disconnect()
        }
    }

    private fun form(vararg fields: Pair<String, String>) =
        fields.joinToString("&") { (k, v) -> "$k=${URLEncoder.encode(v, "UTF-8")}" }

    private fun parseQuery(raw: String?): Map<String, String> =
        raw.orEmpty().split('&').filter { it.contains('=') }.associate {
            val (k, v) = it.split('=', limit = 2)
            k to URLDecoder.decode(v, "UTF-8")
        }

    private fun openBrowser(url: String) {
        val viaDesktop = runCatching {
            if (Desktop.isDesktopSupported() && Desktop.getDesktop().isSupported(Desktop.Action.BROWSE)) {
                Desktop.getDesktop().browse(URI(url)); true
            } else false
        }.getOrDefault(false)
        if (!viaDesktop) {
            val cmd = if (AppDirs.isWindows) listOf("rundll32", "url.dll,FileProtocolHandler", url) else listOf("xdg-open", url)
            ProcessBuilder(cmd).start()
        }
    }
}
