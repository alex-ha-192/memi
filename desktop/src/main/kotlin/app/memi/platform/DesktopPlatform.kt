package app.memi.platform

import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asSkiaBitmap
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.toComposeImageBitmap
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.platform.Font
import androidx.compose.ui.unit.IntRect
import androidx.room.immediateTransaction
import androidx.room.useWriterConnection
import app.memi.data.AppDatabase
import app.memi.sync.DesktopGoogleAuth
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.jetbrains.skia.Codec
import org.jetbrains.skia.Data
import org.jetbrains.skia.EncodedImageFormat
import org.jetbrains.skia.EncodedOrigin
import org.jetbrains.skia.FilterMipmap
import org.jetbrains.skia.FilterMode
import org.jetbrains.skia.Image
import org.jetbrains.skia.MipmapMode
import org.jetbrains.skia.Rect
import org.jetbrains.skia.Surface
import java.awt.FileDialog
import java.awt.Frame
import java.io.File
import java.util.UUID
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// The desktop side of the platform layer described in shared platform/PhotoMemory.kt.

/** Where Memi keeps its database, photos and settings on this computer. */
object AppDirs {
    val isWindows = System.getProperty("os.name").orEmpty().lowercase().contains("win")

    val data: File by lazy {
        val home = File(System.getProperty("user.home"))
        val dir = if (isWindows) {
            File(System.getenv("APPDATA")?.let(::File) ?: File(home, "AppData/Roaming"), "Memi")
        } else {
            File(System.getenv("XDG_DATA_HOME")?.let(::File) ?: File(home, ".local/share"), "memi")
        }
        dir.apply { mkdirs() }
    }
}

val JakartaFamily = FontFamily(
    Font("font/plus_jakarta_sans_regular.ttf", FontWeight.Normal),
    Font("font/plus_jakarta_sans_medium.ttf", FontWeight.Medium),
    Font("font/plus_jakarta_sans_semibold.ttf", FontWeight.SemiBold),
    Font("font/plus_jakarta_sans_bold.ttf", FontWeight.Bold),
    Font("font/plus_jakarta_sans_extrabold.ttf", FontWeight.ExtraBold),
)

@Suppress("DEPRECATION")
@Composable
fun mascotPainter(): Painter = painterResource("memi_mascot.xml")

/** Nothing to tint on desktop: the window has its own title bar. */
@Composable
fun PlatformSystemBars(dark: Boolean) = Unit

suspend fun AppDatabase.inTransaction(block: suspend () -> Unit) {
    useWriterConnection { transactor -> transactor.immediateTransaction { block() } }
}

// --- Back: Escape does what Android's back gesture does ---------------------------------------

object DesktopBack {
    class Handler(var enabled: Boolean, val onBack: () -> Unit)

    private val handlers = mutableListOf<Handler>()

    fun add(h: Handler) { handlers += h }
    fun remove(h: Handler) { handlers -= h }

    /** Runs the most recently added handler that's switched on. True if something handled it. */
    fun back(): Boolean {
        val h = handlers.lastOrNull { it.enabled } ?: return false
        h.onBack()
        return true
    }
}

@Composable
fun PlatformBackHandler(enabled: Boolean = true, onBack: () -> Unit) {
    val current by rememberUpdatedState(onBack)
    val handler = remember { DesktopBack.Handler(enabled) { current() } }
    SideEffect { handler.enabled = enabled }
    DisposableEffect(handler) {
        DesktopBack.add(handler)
        onDispose { DesktopBack.remove(handler) }
    }
}

// --- Sign-in -----------------------------------------------------------------------------------

/** Opens Google's sign-in in your browser; you pick the account there. */
@Composable
fun rememberGoogleSignIn(onToken: (String) -> Unit, onError: (String) -> Unit): () -> Unit {
    val scope = rememberCoroutineScope()
    val token by rememberUpdatedState(onToken)
    val error by rememberUpdatedState(onError)
    return remember {
        {
            scope.launch {
                runCatching { DesktopGoogleAuth.signIn() }
                    .onSuccess { token(it) }
                    .onFailure { error(it.message ?: "Couldn't sign in to Google.") }
            }
        }
    }
}

// --- Photos ------------------------------------------------------------------------------------

/** A file dialog for picking a photo. Returns a function that opens it. */
@Composable
fun rememberPhotoPicker(onLoading: (Boolean) -> Unit, onPicked: (ImageBitmap?) -> Unit): () -> Unit {
    val scope = rememberCoroutineScope()
    val loading by rememberUpdatedState(onLoading)
    val picked by rememberUpdatedState(onPicked)
    return remember {
        {
            val dialog = FileDialog(null as Frame?, "Choose a photo", FileDialog.LOAD).apply {
                isMultipleMode = false
                setFilenameFilter { _, name -> name.substringAfterLast('.').lowercase() in PlatformPhotos.extensions }
                if (AppDirs.isWindows) file = PlatformPhotos.extensions.joinToString(";") { "*.$it" }
            }
            dialog.isVisible = true
            val file = dialog.files.firstOrNull()
            if (file != null) scope.launch {
                loading(true)
                picked(PlatformPhotos.loadPicked(file))
                loading(false)
            }
        }
    }
}

/** Profile photos: picked, cropped in the app, and kept in Memi's folder on this computer. */
object PlatformPhotos {
    val extensions = setOf("jpg", "jpeg", "png", "webp", "bmp", "gif")
    private const val LOAD_EDGE = 2048
    private const val SAVE_WIDTH = 1080
    private val smooth = FilterMipmap(FilterMode.LINEAR, MipmapMode.NONE)

    val dir: File get() = File(AppDirs.data, "photos")

    suspend fun loadFile(path: String): ImageBitmap? = withContext(Dispatchers.IO) {
        runCatching { Image.makeFromEncoded(File(path).readBytes()).toComposeImageBitmap() }.getOrNull()
    }

    /** Decodes a picked photo, turns it the right way up, and shrinks it to a comfortable size. */
    suspend fun loadPicked(file: File): ImageBitmap? = withContext(Dispatchers.IO) {
        runCatching {
            val bytes = file.readBytes()
            val image = Image.makeFromEncoded(bytes)
            val degrees = runCatching {
                when (Codec.makeFromData(Data.makeFromBytes(bytes)).encodedOrigin) {
                    EncodedOrigin.RIGHT_TOP -> 90f
                    EncodedOrigin.BOTTOM_RIGHT -> 180f
                    EncodedOrigin.LEFT_BOTTOM -> 270f
                    else -> 0f
                }
            }.getOrDefault(0f)
            val w = image.width.toFloat()
            val h = image.height.toFloat()
            val scale = min(1f, LOAD_EDGE / max(w, h))
            val sideways = degrees == 90f || degrees == 270f
            val outW = ((if (sideways) h else w) * scale).roundToInt()
            val outH = ((if (sideways) w else h) * scale).roundToInt()
            val surface = Surface.makeRasterN32Premul(outW, outH)
            surface.canvas.apply {
                translate(outW / 2f, outH / 2f)
                rotate(degrees)
                drawImageRect(
                    image, Rect.makeWH(w, h),
                    Rect.makeXYWH(-w * scale / 2f, -h * scale / 2f, w * scale, h * scale),
                    smooth, null, true,
                )
            }
            surface.makeImageSnapshot().toComposeImageBitmap()
        }.getOrNull()
    }

    suspend fun saveCrop(image: ImageBitmap, crop: IntRect): String? = withContext(Dispatchers.IO) {
        runCatching {
            val source = Image.makeFromBitmap(image.asSkiaBitmap())
            val outW = min(crop.width, SAVE_WIDTH)
            val outH = (crop.height * outW.toFloat() / crop.width).roundToInt()
            val surface = Surface.makeRasterN32Premul(outW, outH)
            surface.canvas.drawImageRect(
                source,
                Rect.makeXYWH(crop.left.toFloat(), crop.top.toFloat(), crop.width.toFloat(), crop.height.toFloat()),
                Rect.makeWH(outW.toFloat(), outH.toFloat()),
                smooth, null, true,
            )
            val jpeg = surface.makeImageSnapshot().encodeToData(EncodedImageFormat.JPEG, 90) ?: error("Couldn't save photo")
            dir.mkdirs()
            val file = File(dir, "${UUID.randomUUID()}.jpg")
            file.writeBytes(jpeg.bytes)
            file.absolutePath
        }.getOrNull()
    }

    fun delete(path: String?) {
        if (path != null) runCatching { File(path).delete() }
    }
}

/** Escape closes the sheet or dialog on top. */
@Composable
fun DismissOnEscape(onDismiss: () -> Unit) = PlatformBackHandler(true, onDismiss)

/** Computers show the code; they don't scan one. */
@Composable
fun rememberQrScanner(onScanned: (String) -> Unit, onError: (String) -> Unit): (() -> Unit)? = null
