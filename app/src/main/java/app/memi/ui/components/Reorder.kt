package app.memi.ui.components

import androidx.compose.foundation.border
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.unit.dp
import androidx.compose.ui.zIndex
import app.memi.ui.theme.Bento

/**
 * Hold, then move, to reorder: the item lifts and follows your finger, the one it's over is
 * outlined, and letting go puts it in that one's place. Holding and letting go without moving
 * opens the item's menu, as before (see [pressable]).
 *
 * One state per list; each item uses [reorderable] for the visuals and passes [handle] to
 * [pressable] for the gesture.
 */
class ReorderState<K : Any> internal constructor(private val onMove: (from: K, to: K) -> Unit) {
    var dragging by mutableStateOf<K?>(null)
        private set
    var offset by mutableStateOf(Offset.Zero)
        private set
    var target by mutableStateOf<K?>(null)
        private set
    internal val bounds = mutableMapOf<K, Rect>()
    private var grab = Offset.Zero

    fun handle(key: K) = object : ReorderHandle {
        override fun start(pointerInItem: Offset) {
            dragging = key
            offset = Offset.Zero
            target = null
            grab = (bounds[key]?.topLeft ?: Offset.Zero) + pointerInItem
        }

        override fun drag(delta: Offset) {
            offset += delta
            val at = grab + offset
            target = bounds.entries.firstOrNull { it.key != key && it.value.contains(at) }?.key
        }

        override fun end() {
            val to = target
            reset()
            if (to != null && to != key) onMove(key, to)
        }

        override fun cancel() = reset()
    }

    private fun reset() {
        dragging = null
        target = null
        offset = Offset.Zero
    }
}

/** What [pressable] tells the list as a drag goes on. */
interface ReorderHandle {
    fun start(pointerInItem: Offset)
    fun drag(delta: Offset)
    fun end()
    fun cancel()
}

@Composable
fun <K : Any> rememberReorderState(onMove: (from: K, to: K) -> Unit): ReorderState<K> {
    val latest by rememberUpdatedState(onMove)
    return remember { ReorderState<K> { a, b -> latest(a, b) } }
}

/** The look of a reorderable item: lifted while dragged, outlined while something's over it. */
@Composable
fun <K : Any> Modifier.reorderable(state: ReorderState<K>, key: K, itemShape: Shape): Modifier {
    DisposableEffect(state, key) { onDispose { state.bounds.remove(key) } }
    val lifted = state.dragging == key
    val over = state.target == key
    return this
        .onGloballyPositioned { state.bounds[key] = it.boundsInRoot() }
        .zIndex(if (lifted) 1f else 0f)
        .graphicsLayer {
            if (lifted) {
                translationX = state.offset.x
                translationY = state.offset.y
                scaleX = 1.04f
                scaleY = 1.04f
                shadowElevation = 18.dp.toPx()
                shape = itemShape
                clip = false
            }
        }
        .then(if (over) Modifier.border(2.dp, Bento.colors.ink, itemShape) else Modifier)
}
