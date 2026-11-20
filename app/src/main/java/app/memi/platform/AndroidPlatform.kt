package app.memi.platform

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Color as AndroidColor
import android.graphics.ImageDecoder
import android.graphics.Matrix
import android.net.Uri
import android.os.Build
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import com.google.mlkit.vision.barcode.common.Barcode
import com.google.mlkit.vision.codescanner.GmsBarcodeScannerOptions
import com.google.mlkit.vision.codescanner.GmsBarcodeScanning
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntRect
import androidx.exifinterface.media.ExifInterface
import app.memi.R
import app.memi.sync.GoogleAuth
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.UUID
import kotlin.math.max
import kotlin.math.roundToInt

// Android's side of the platform layer described in shared platform/PhotoMemory.kt.

val JakartaFamily = FontFamily(
    Font(R.font.plus_jakarta_sans_regular, FontWeight.Normal),
    Font(R.font.plus_jakarta_sans_medium, FontWeight.Medium),
    Font(R.font.plus_jakarta_sans_semibold, FontWeight.SemiBold),
    Font(R.font.plus_jakarta_sans_bold, FontWeight.Bold),
    Font(R.font.plus_jakarta_sans_extrabold, FontWeight.ExtraBold),
)

@Composable
fun mascotPainter(): Painter = painterResource(R.drawable.memi_mascot)

@Composable
fun PlatformBackHandler(enabled: Boolean = true, onBack: () -> Unit) = BackHandler(enabled, onBack)

/** Keeps the status and navigation bar icons readable as the theme changes between light and dark. */
@Composable
fun PlatformSystemBars(dark: Boolean) {
    val activity = LocalContext.current as? ComponentActivity ?: return
    LaunchedEffect(dark) {
        val style = if (dark) SystemBarStyle.dark(AndroidColor.TRANSPARENT)
        else SystemBarStyle.light(AndroidColor.TRANSPARENT, AndroidColor.TRANSPARENT)
        activity.enableEdgeToEdge(statusBarStyle = style, navigationBarStyle = style)
    }
}

/** Android's photo picker. Returns a function that opens it. */
@Composable
fun rememberPhotoPicker(onLoading: (Boolean) -> Unit, onPicked: (ImageBitmap?) -> Unit): () -> Unit {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
        if (uri != null) scope.launch {
            onLoading(true)
            onPicked(PlatformPhotos.loadPicked(context, uri))
            onLoading(false)
        }
    }
    return { launcher.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }
}

/** Google's own account picker and consent screen. Returns a function that starts sign-in. */
@Composable
fun rememberGoogleSignIn(onToken: (String) -> Unit, onError: (String) -> Unit): () -> Unit {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val consent = rememberLauncherForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        val token = GoogleAuth.tokenFromIntent(context, result.data)
        if (token != null) onToken(token) else onError("Sign-in was cancelled.")
    }
    return {
        scope.launch {
            runCatching { GoogleAuth.authorize(context) }
                .onSuccess { o ->
                    when (o) {
                        is GoogleAuth.Outcome.Token -> onToken(o.value)
                        is GoogleAuth.Outcome.NeedsConsent -> consent.launch(IntentSenderRequest.Builder(o.intent).build())
                    }
                }
                .onFailure { onError("Couldn't reach Google: ${it.message}") }
        }
    }
}

/** Profile photos: picked, cropped in the app, and kept in the app's private storage. */
object PlatformPhotos {
    /** Large enough to crop comfortably, small enough to stay quick. */
    private const val LOAD_EDGE = 2048
    /** Saved photos are at most this wide (4:5, so 1350 tall). */
    private const val SAVE_WIDTH = 1080

    lateinit var dir: File
        private set

    fun init(context: Context) {
        dir = File(context.filesDir, "photos")
    }

    suspend fun loadFile(path: String): ImageBitmap? = withContext(Dispatchers.IO) {
        runCatching { BitmapFactory.decodeFile(path)?.asImageBitmap() }.getOrNull()
    }

    suspend fun saveCrop(image: ImageBitmap, crop: IntRect): String? = withContext(Dispatchers.IO) {
        runCatching {
            val source = image.asAndroidBitmap()
            val cropped = Bitmap.createBitmap(source, crop.left, crop.top, crop.width, crop.height)
            val scaled = if (cropped.width > SAVE_WIDTH) {
                val h = (cropped.height * SAVE_WIDTH.toFloat() / cropped.width).roundToInt()
                Bitmap.createScaledBitmap(cropped, SAVE_WIDTH, h, true)
            } else cropped
            dir.mkdirs()
            val file = File(dir, "${UUID.randomUUID()}.jpg")
            file.outputStream().use { scaled.compress(Bitmap.CompressFormat.JPEG, 90, it) }
            file.absolutePath
        }.getOrNull()
    }

    fun delete(path: String?) {
        if (path != null) runCatching { File(path).delete() }
    }

    suspend fun loadPicked(context: Context, uri: Uri): ImageBitmap? = withContext(Dispatchers.IO) {
        runCatching { decodeUri(context, uri)?.asImageBitmap() }.getOrNull()
    }

    private fun decodeUri(context: Context, uri: Uri): Bitmap? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            // ImageDecoder applies the photo's rotation for us.
            val source = ImageDecoder.createSource(context.contentResolver, uri)
            ImageDecoder.decodeBitmap(source) { decoder, info, _ ->
                val w = info.size.width
                val h = info.size.height
                val longest = max(w, h)
                if (longest > LOAD_EDGE) {
                    val scale = LOAD_EDGE.toFloat() / longest
                    decoder.setTargetSize((w * scale).toInt(), (h * scale).toInt())
                }
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            }
        } else {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
            var sample = 1
            while (max(bounds.outWidth, bounds.outHeight) / (sample * 2) >= LOAD_EDGE) sample *= 2
            val opts = BitmapFactory.Options().apply { inSampleSize = sample }
            val raw = context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, opts) } ?: return null
            val degrees = context.contentResolver.openInputStream(uri)?.use {
                when (ExifInterface(it).getAttributeInt(ExifInterface.TAG_ORIENTATION, ExifInterface.ORIENTATION_NORMAL)) {
                    ExifInterface.ORIENTATION_ROTATE_90 -> 90f
                    ExifInterface.ORIENTATION_ROTATE_180 -> 180f
                    ExifInterface.ORIENTATION_ROTATE_270 -> 270f
                    else -> 0f
                }
            } ?: 0f
            if (degrees == 0f) raw
            else Bitmap.createBitmap(raw, 0, 0, raw.width, raw.height, Matrix().apply { postRotate(degrees) }, true)
        }
    }
}

/** Sheets and dialogs already close with the back gesture on Android. */
@Composable
fun DismissOnEscape(onDismiss: () -> Unit) = Unit

/**
 * Google's ready-made QR scanner (from Play services, so Memi needs no camera permission).
 * Returns a function that opens it.
 */
@Composable
fun rememberQrScanner(onScanned: (String) -> Unit, onError: (String) -> Unit): (() -> Unit)? {
    val context = LocalContext.current
    val scanned by rememberUpdatedState(onScanned)
    val error by rememberUpdatedState(onError)
    return remember(context) {
        {
            val options = GmsBarcodeScannerOptions.Builder().setBarcodeFormats(Barcode.FORMAT_QR_CODE).build()
            GmsBarcodeScanning.getClient(context, options).startScan()
                .addOnSuccessListener { code ->
                    val text = code.rawValue
                    if (text != null) scanned(text) else error("Couldn't read that code. Try again.")
                }
                .addOnFailureListener { error("Couldn't open the scanner: ${it.message}") }
        }
    }
}
