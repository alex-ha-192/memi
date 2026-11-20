package app.memi.ui.theme

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FiniteAnimationSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import app.memi.data.Appearance
import app.memi.data.ThemeChoice

/**
 * Motion used across the app: gentle easing, but quick enough to feel responsive.
 * Springs settle without bouncing; colour changes take about a third of a second.
 */
object Motion {
    val CozyEase = CubicBezierEasing(0.3f, 0f, 0.2f, 1f)
    fun <T> colour(): FiniteAnimationSpec<T> = tween(durationMillis = 320, easing = CozyEase)
    fun <T> settle(): FiniteAnimationSpec<T> = spring(dampingRatio = 0.86f, stiffness = Spring.StiffnessMedium)
    fun <T> soft(): FiniteAnimationSpec<T> = tween(durationMillis = 200, easing = CozyEase)
}

/** Plus Jakarta Sans, loaded by each app from its own resources. */
val Jakarta = app.memi.platform.JakartaFamily

object BentoType {
    val display = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.ExtraBold, fontSize = 32.sp, lineHeight = 38.sp, letterSpacing = (-0.9).sp)
    val tileTitle = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.ExtraBold, fontSize = 22.sp, lineHeight = 26.sp, letterSpacing = (-0.4).sp)
    val section = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.ExtraBold, fontSize = 19.sp, lineHeight = 24.sp)
    val cardTitle = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.ExtraBold, fontSize = 15.sp, lineHeight = 20.sp)
    val itemTitle = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.Bold, fontSize = 15.sp, lineHeight = 20.sp)
    val body = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.SemiBold, fontSize = 14.sp, lineHeight = 20.sp)
    val label = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.SemiBold, fontSize = 13.sp, lineHeight = 17.sp)
    val caption = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.SemiBold, fontSize = 12.sp, lineHeight = 16.sp)
    val tag = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.ExtraBold, fontSize = 12.sp, lineHeight = 16.sp)
    val input = TextStyle(fontFamily = Jakarta, fontWeight = FontWeight.SemiBold, fontSize = 15.sp, lineHeight = 20.sp)
}

val LocalBento = staticCompositionLocalOf { Schemes.Bento.light }

/** Shortcut: `Bento.colors.ink`. */
object Bento {
    val colors: BentoColors
        @Composable get() = LocalBento.current
}

@Composable
fun BentoTheme(choice: ThemeChoice, content: @Composable () -> Unit) {
    val systemDark = isSystemInDarkTheme()
    val dark = when (choice.appearance) {
        Appearance.LIGHT -> false
        Appearance.DARK -> true
        Appearance.SYSTEM -> systemDark
    }
    val scheme = Schemes.byName(choice.scheme)
    val target = if (dark) scheme.dark else scheme.light

    // Switching theme used to ease ~60 colours separately, which re-laid out the whole app every
    // frame. Now the old screen is snapshotted once, the new colours apply straight away, and the
    // snapshot fades out on top. Only the fade's alpha changes per frame, so it stays smooth.
    val layer = rememberGraphicsLayer()
    var colors by remember { mutableStateOf(target) }
    var snapshot by remember { mutableStateOf<ImageBitmap?>(null) }
    val fade = remember { Animatable(0f) }
    LaunchedEffect(target) {
        if (target == colors) return@LaunchedEffect
        val old = if (layer.size.width > 0 && layer.size.height > 0) runCatching { layer.toImageBitmap() }.getOrNull() else null
        snapshot = old
        fade.snapTo(1f)
        colors = target
        if (old != null) fade.animateTo(0f, tween(360, easing = Motion.CozyEase))
        snapshot = null
    }

    val material = if (dark) {
        darkColorScheme(
            primary = colors.ink, onPrimary = colors.bg,
            background = colors.bg, onBackground = colors.ink,
            surface = colors.sheet, onSurface = colors.ink,
            surfaceContainer = colors.sheet, surfaceContainerHigh = colors.sheet, surfaceContainerLow = colors.sheet,
            surfaceVariant = colors.track, onSurfaceVariant = colors.muted,
            outline = colors.line, error = colors.danger,
        )
    } else {
        lightColorScheme(
            primary = colors.ink, onPrimary = Color.White,
            background = colors.bg, onBackground = colors.ink,
            surface = colors.sheet, onSurface = colors.ink,
            surfaceContainer = colors.sheet, surfaceContainerHigh = colors.sheet, surfaceContainerLow = colors.sheet,
            surfaceVariant = colors.track, onSurfaceVariant = colors.muted,
            outline = colors.line, error = colors.danger,
        )
    }

    CompositionLocalProvider(LocalBento provides colors) {
        MaterialTheme(colorScheme = material) {
            Box(
                Modifier.fillMaxSize().drawWithContent {
                    layer.record { this@drawWithContent.drawContent() }
                    drawLayer(layer)
                    snapshot?.let { drawImage(it, alpha = fade.value) }
                }
            ) { content() }
        }
    }
}


