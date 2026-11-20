package app.memi.platform

import androidx.compose.runtime.Composable
import androidx.compose.runtime.produceState
import androidx.compose.ui.graphics.ImageBitmap

/**
 * Each app provides the same small set of platform pieces under app.memi.platform:
 *
 * - PlatformPhotos: loading, cropping and saving profile photos
 * - rememberPhotoPicker: the system photo picker (Android) or a file dialog (desktop)
 * - PlatformBackHandler: Android's back gesture; Escape on desktop
 * - PlatformSystemBars: status bar icon colours (Android only)
 * - JakartaFamily and mascotPainter: fonts and the elephant, from each app's resources
 * - rememberGoogleSignIn: Google's account picker (Android) or a browser sign-in (desktop)
 *
 * This file is the one shared piece: a small in-memory cache, so photos don't decode again every
 * time a card scrolls back into view.
 */
private object PhotoMemory {
    private const val MAX = 48
    private val cache = object : LinkedHashMap<String, ImageBitmap>(MAX, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, ImageBitmap>?) = size > MAX
    }

    @Synchronized fun get(path: String): ImageBitmap? = cache[path]
    @Synchronized fun put(path: String, image: ImageBitmap) { cache[path] = image }
}

/** A saved profile photo, loaded off the main thread and remembered. Null while loading or if missing. */
@Composable
fun rememberPhoto(path: String): ImageBitmap? {
    val cached = PhotoMemory.get(path)
    if (cached != null) return cached
    return produceState<ImageBitmap?>(null, path) {
        value = PlatformPhotos.loadFile(path)?.also { PhotoMemory.put(path, it) }
    }.value
}
