package app.memi.ui.sheets

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.unit.IntRect
import androidx.compose.ui.input.pointer.PointerEventType
import app.memi.ui.components.LocalHasMouse
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTransformGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import kotlin.math.max
import kotlin.math.roundToInt

/** Profile photos are cropped to this shape (width ÷ height): slightly portrait. */
const val PhotoAspect = 4f / 5f

/**
 * Drag to move and pinch to zoom inside a 4:5 frame. The photo always covers the frame,
 * so there are never empty edges.
 */
@Composable
fun CropDialog(image: ImageBitmap, onCancel: () -> Unit, onDone: (IntRect) -> Unit) {
    val colors = Bento.colors
    val bitmap = image
    var zoom by remember { mutableFloatStateOf(1f) }
    var offset by remember { mutableStateOf(Offset.Zero) }
    val density = LocalDensity.current
    app.memi.platform.DismissOnEscape(onCancel)

    Dialog(
        onDismissRequest = onCancel,
        properties = DialogProperties(usePlatformDefaultWidth = false),
    ) {
        Column(Modifier.fillMaxSize().background(Color(0xFF111015)).statusBarsPadding().navigationBarsPadding()) {
            Text(
                if (LocalHasMouse.current) "Drag to move, scroll to zoom" else "Drag and pinch to frame them",
                style = BentoType.itemTitle,
                color = Color.White,
                modifier = Modifier.align(Alignment.CenterHorizontally).padding(top = 20.dp, bottom = 8.dp),
            )
            BoxWithConstraints(Modifier.weight(1f).fillMaxWidth()) {
                val areaW = constraints.maxWidth.toFloat()
                val areaH = constraints.maxHeight.toFloat()
                val margin = with(density) { 24.dp.toPx() }
                // Leave room for the buttons at the bottom.
                val usableH = areaH - with(density) { 88.dp.toPx() }
                val frameW = minOf(areaW - margin * 2, (usableH - margin * 2) * PhotoAspect)
                val frameH = frameW / PhotoAspect
                val frameLeft = (areaW - frameW) / 2f
                val frameTop = (usableH - frameH) / 2f
                val baseScale = max(frameW / bitmap.width, frameH / bitmap.height)

                fun clamp(o: Offset, z: Float): Offset {
                    val dw = bitmap.width * baseScale * z
                    val dh = bitmap.height * baseScale * z
                    val mx = (dw - frameW) / 2f
                    val my = (dh - frameH) / 2f
                    return Offset(o.x.coerceIn(-mx, mx), o.y.coerceIn(-my, my))
                }
                val clampFn by rememberUpdatedState(::clamp)

                Canvas(
                    Modifier
                        .fillMaxSize()
                        .pointerInput(Unit) {
                            detectTransformGestures { _, pan, zoomChange, _ ->
                                val z = (zoom * zoomChange).coerceIn(1f, 5f)
                                zoom = z
                                offset = clampFn(offset + pan, z)
                            }
                        }
                        // With a mouse, the scroll wheel zooms.
                        .pointerInput(Unit) {
                            awaitPointerEventScope {
                                while (true) {
                                    val event = awaitPointerEvent()
                                    if (event.type == PointerEventType.Scroll) {
                                        val dy = event.changes.firstOrNull()?.scrollDelta?.y ?: 0f
                                        val z = (zoom * (1f - dy * 0.1f)).coerceIn(1f, 5f)
                                        zoom = z
                                        offset = clampFn(offset, z)
                                        event.changes.forEach { it.consume() }
                                    }
                                }
                            }
                        }
                ) {
                    val scale = baseScale * zoom
                    val dw = bitmap.width * scale
                    val dh = bitmap.height * scale
                    val left = frameLeft + (frameW - dw) / 2f + offset.x
                    val top = frameTop + (frameH - dh) / 2f + offset.y
                    drawImage(
                        image,
                        srcOffset = IntOffset.Zero,
                        srcSize = IntSize(bitmap.width, bitmap.height),
                        dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                        dstSize = IntSize(dw.roundToInt(), dh.roundToInt()),
                        filterQuality = FilterQuality.Medium,
                    )
                    clipRect(frameLeft, frameTop, frameLeft + frameW, frameTop + frameH, clipOp = ClipOp.Difference) {
                        drawRect(Color.Black.copy(alpha = 0.62f))
                    }
                    drawRoundRect(
                        color = Color.White,
                        topLeft = Offset(frameLeft, frameTop),
                        size = Size(frameW, frameH),
                        cornerRadius = CornerRadius(28.dp.toPx()),
                        style = Stroke(width = 2.dp.toPx()),
                    )
                }

                // Saves exactly what's inside the frame.
                val crop = {
                    val scale = baseScale * zoom
                    val dw = bitmap.width * scale
                    val dh = bitmap.height * scale
                    val imgLeft = frameLeft + (frameW - dw) / 2f + offset.x
                    val imgTop = frameTop + (frameH - dh) / 2f + offset.y
                    val x = ((frameLeft - imgLeft) / scale).roundToInt().coerceIn(0, bitmap.width - 1)
                    val y = ((frameTop - imgTop) / scale).roundToInt().coerceIn(0, bitmap.height - 1)
                    val w = (frameW / scale).roundToInt().coerceIn(1, bitmap.width - x)
                    val h = (frameH / scale).roundToInt().coerceIn(1, bitmap.height - y)
                    IntRect(x, y, x + w, y + h)
                }
                // Two equal buttons, lined up with the crop frame's edges.
                val frameWidthDp = with(density) { frameW.toDp() }
                Row(
                    Modifier.align(Alignment.BottomCenter).padding(bottom = 16.dp).width(frameWidthDp),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Box(
                        Modifier.weight(1f).height(54.dp).clip(RoundedCornerShape(18.dp)).background(Color.White.copy(alpha = 0.12f))
                            .clickable(role = Role.Button, onClick = onCancel),
                        contentAlignment = Alignment.Center,
                    ) { Text("Cancel", style = BentoType.cardTitle, color = Color.White, textAlign = TextAlign.Center) }
                    Box(
                        Modifier.weight(1f).height(54.dp).clip(RoundedCornerShape(18.dp)).background(colors.gift)
                            .clickable(role = Role.Button) { onDone(crop()) },
                        contentAlignment = Alignment.Center,
                    ) { Text("Use photo", style = BentoType.cardTitle, color = colors.giftInk, textAlign = TextAlign.Center) }
                }
            }
        }
    }
}
