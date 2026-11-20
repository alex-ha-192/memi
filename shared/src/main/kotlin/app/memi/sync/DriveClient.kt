package app.memi.sync

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONObject
import java.io.ByteArrayOutputStream
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder

/**
 * The few Google Drive calls Memi needs. Everything lives in the hidden app data folder, which
 * only Memi can see: it doesn't show up in your Drive and Memi can't see your other files.
 */
class DriveClient(private val token: String) {
    class Unauthorized : Exception("Google sign-in expired")
    class HttpError(val code: Int, body: String) : Exception("Google Drive said $code: ${body.take(200)}")

    data class RemoteFile(val id: String, val modifiedTime: String?, val size: Long?)

    suspend fun find(name: String): RemoteFile? = withContext(Dispatchers.IO) {
        val q = URLEncoder.encode("name='$name' and trashed=false", "UTF-8")
        val url = "$API/files?spaces=appDataFolder&q=$q&fields=files(id,modifiedTime,size)&orderBy=modifiedTime%20desc"
        val files = JSONObject(String(request("GET", url))).getJSONArray("files")
        if (files.length() == 0) null else files.getJSONObject(0).let {
            RemoteFile(it.getString("id"), it.optString("modifiedTime").ifEmpty { null }, it.optString("size").toLongOrNull())
        }
    }

    /** The email of the Google account that's connected, to show in settings. */
    suspend fun email(): String? = withContext(Dispatchers.IO) {
        runCatching { JSONObject(String(request("GET", "$API/about?fields=user(emailAddress)"))).getJSONObject("user").optString("emailAddress").ifEmpty { null } }.getOrNull()
    }

    suspend fun download(id: String): ByteArray = withContext(Dispatchers.IO) { request("GET", "$API/files/$id?alt=media") }

    suspend fun create(name: String, bytes: ByteArray): RemoteFile = withContext(Dispatchers.IO) {
        val boundary = "memi${System.nanoTime()}"
        val meta = JSONObject().put("name", name).put("parents", org.json.JSONArray().put("appDataFolder")).toString()
        val body = ByteArrayOutputStream().apply {
            write("--$boundary\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n$meta\r\n".toByteArray())
            write("--$boundary\r\nContent-Type: application/octet-stream\r\n\r\n".toByteArray())
            write(bytes)
            write("\r\n--$boundary--\r\n".toByteArray())
        }.toByteArray()
        parse(request("POST", "$UPLOAD/files?uploadType=multipart&fields=id,modifiedTime,size", body, "multipart/related; boundary=$boundary"))
    }

    suspend fun update(id: String, bytes: ByteArray): RemoteFile = withContext(Dispatchers.IO) {
        // HttpURLConnection has no PATCH, so use Google's method override header.
        parse(request("POST", "$UPLOAD/files/$id?uploadType=media&fields=id,modifiedTime,size", bytes, "application/octet-stream", override = "PATCH"))
    }

    private fun parse(res: ByteArray): RemoteFile = JSONObject(String(res)).let {
        RemoteFile(it.getString("id"), it.optString("modifiedTime").ifEmpty { null }, it.optString("size").toLongOrNull())
    }

    private fun request(method: String, url: String, body: ByteArray? = null, type: String? = null, override: String? = null): ByteArray {
        val c = URL(url).openConnection() as HttpURLConnection
        try {
            c.requestMethod = method
            c.connectTimeout = 20_000
            c.readTimeout = 60_000
            c.setRequestProperty("Authorization", "Bearer $token")
            override?.let { c.setRequestProperty("X-HTTP-Method-Override", it) }
            if (body != null) {
                c.doOutput = true
                c.setRequestProperty("Content-Type", type ?: "application/octet-stream")
                c.setFixedLengthStreamingMode(body.size)
                c.outputStream.use { it.write(body) }
            }
            val code = c.responseCode
            if (code == 401) throw Unauthorized()
            if (code !in 200..299) throw HttpError(code, c.errorStream?.use { String(it.readBytes()) }.orEmpty())
            return c.inputStream.use { it.readBytes() }
        } finally {
            c.disconnect()
        }
    }

    private companion object {
        const val API = "https://www.googleapis.com/drive/v3"
        const val UPLOAD = "https://www.googleapis.com/upload/drive/v3"
    }
}
