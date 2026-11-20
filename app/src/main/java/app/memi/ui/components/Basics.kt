@file:OptIn(ExperimentalFoundationApi::class)

package app.memi.ui.components

import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.PointerEventType
import androidx.compose.ui.input.pointer.PointerType
import androidx.compose.ui.input.pointer.isSecondaryPressed
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChange
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.awaitLongPressOrCancellation
import androidx.compose.foundation.gestures.drag
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.onLongClick
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.memi.data.Person
import app.memi.ui.Toast
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import app.memi.ui.theme.Motion
import androidx.compose.foundation.Image
import androidx.compose.ui.graphics.FilterQuality

/** True once the person is using a mouse or trackpad, so hover affordances can appear. */
val LocalHasMouse = compositionLocalOf { false }

/** Watches every pointer event (without consuming it) to know whether a mouse is in use. */
@Composable
fun PointerModeProvider(content: @Composable () -> Unit) {
    var hasMouse by remember { mutableStateOf(false) }
    Box(
        Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent(PointerEventPass.Initial)
                        val type = event.changes.firstOrNull()?.type ?: continue
                        if (type == PointerType.Mouse) hasMouse = true
                        else if ((type == PointerType.Touch || type == PointerType.Stylus) && event.type == PointerEventType.Press) hasMouse = false
                    }
                }
            }
    ) {
        CompositionLocalProvider(LocalHasMouse provides hasMouse) { content() }
    }
}

/**
 * Tap to open, press and hold (or right-click) for the menu.
 * Callbacks are read through state, so the gesture detectors are set up once and never restarted.
 */
@Composable
fun Modifier.pressable(
    onClick: (() -> Unit)?,
    onMenu: (() -> Unit)?,
    interaction: MutableInteractionSource,
    /** Set to make the item draggable: hold, then move. Holding and letting go opens [onMenu]. */
    reorder: ReorderHandle? = null,
): Modifier {
    val click by rememberUpdatedState(onClick)
    val menu by rememberUpdatedState(onMenu)
    val handle by rememberUpdatedState(reorder)
    val hasMenu = onMenu != null
    val canDrag = reorder != null
    val haptics = LocalHapticFeedback.current
    // When this gesture started, and when it became a long press: a long press isn't also a tap.
    val times = remember { LongArray(2) }
    return this
        .pointerInput(hasMenu) {
            if (!hasMenu) return@pointerInput
            awaitPointerEventScope {
                while (true) {
                    // Initial pass, so a right-click opens the menu without also counting as a tap.
                    val event = awaitPointerEvent(PointerEventPass.Initial)
                    if (event.type == PointerEventType.Press && event.buttons.isSecondaryPressed) {
                        event.changes.forEach { it.consume() }
                        menu?.invoke()
                    }
                }
            }
        }
        .then(
            if (!canDrag) Modifier else Modifier
                .pointerInput(Unit) {
                    awaitEachGesture {
                        val down = awaitFirstDown(requireUnconsumed = false)
                        times[0] = down.uptimeMillis
                        val long = awaitLongPressOrCancellation(down.id) ?: return@awaitEachGesture
                        times[1] = long.uptimeMillis
                        haptics.performHapticFeedback(HapticFeedbackType.LongPress)
                        val h = handle ?: return@awaitEachGesture
                        h.start(long.position)
                        var total = Offset.Zero
                        var moved = false
                        val finished = drag(long.id) { change ->
                            val delta = change.positionChange()
                            total += delta
                            if (!moved && total.getDistance() > viewConfiguration.touchSlop) {
                                moved = true
                                h.drag(total)
                            } else if (moved) h.drag(delta)
                            change.consume()
                        }
                        when {
                            !moved -> { h.cancel(); menu?.invoke() }
                            finished -> h.end()
                            else -> h.cancel()
                        }
                    }
                }
                .semantics { if (hasMenu) onLongClick("Menu") { menu?.invoke(); true } }
        )
        .combinedClickable(
            interactionSource = interaction,
            indication = null,
            role = Role.Button,
            onClick = { if (!(canDrag && times[1] > times[0])) click?.invoke() },
            onLongClick = if (hasMenu && !canDrag) ({ menu?.invoke() }) else null,
        )
}

/** Press feedback that animates in the draw phase only, so it never recomposes the content. */
@Composable
fun Modifier.pressScale(interaction: MutableInteractionSource, pressedScale: Float = 0.975f): Modifier {
    val pressed by interaction.collectIsPressedAsState()
    val scale = animateFloatAsState(if (pressed) pressedScale else 1f, Motion.settle(), label = "press")
    return this.graphicsLayer {
        scaleX = scale.value
        scaleY = scale.value
    }
}

/** A rounded surface that gently presses in when touched. */
@Composable
fun Tile(
    modifier: Modifier = Modifier,
    color: Color,
    shape: Shape = RoundedCornerShape(24.dp),
    onClick: (() -> Unit)? = null,
    onMenu: (() -> Unit)? = null,
    contentPadding: Dp = 16.dp,
    reorder: ReorderHandle? = null,
    content: @Composable BoxScope.() -> Unit,
) {
    val interaction = remember { MutableInteractionSource() }
    val base = modifier
        .pressScale(interaction)
        .clip(shape)
        .background(color)
    val clickable = if (onClick != null || onMenu != null) base.pressable(onClick, onMenu, interaction, reorder) else base
    Box(clickable.padding(contentPadding), content = content)
}

/** A person's photo, or a soft placeholder in their tone when there isn't one yet. */
@Composable
fun PersonPhoto(person: Person?, modifier: Modifier = Modifier, shape: Shape = RoundedCornerShape(16.dp)) {
    val colors = Bento.colors
    val n = colors.tones.size
    val tone = colors.tones[(((person?.toneIndex ?: 0) % n) + n) % n]
    Box(modifier.clip(shape).background(tone), contentAlignment = Alignment.BottomCenter) {
        val path = person?.photoPath
        if (path != null) {
            val image = app.memi.platform.rememberPhoto(path)
            if (image != null) {
                Image(
                    bitmap = image,
                    contentDescription = person.name,
                    contentScale = ContentScale.Crop,
                    filterQuality = FilterQuality.Medium,
                    modifier = Modifier.fillMaxSize(),
                )
            }
        } else {
            Icon(
                Icons.Rounded.Person,
                contentDescription = null,
                tint = Color.White.copy(alpha = 0.6f),
                modifier = Modifier.fillMaxSize(0.92f),
            )
        }
    }
}

@Composable
fun TagChip(text: String, background: Color, ink: Color, modifier: Modifier = Modifier) {
    Box(
        modifier
            .clip(RoundedCornerShape(11.dp))
            .background(background)
            .padding(horizontal = 10.dp, vertical = 6.dp)
    ) {
        Text(text, style = BentoType.tag, color = ink, maxLines = 1)
    }
}

@Composable
fun SectionHeader(title: String, modifier: Modifier = Modifier, trailing: String? = null, count: Int? = null) {
    Row(modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(title, style = BentoType.section, color = Bento.colors.ink)
        if (count != null) {
            Text("  $count", style = BentoType.section.copy(fontWeight = FontWeight.SemiBold), color = Bento.colors.muted)
        }
        Spacer(Modifier.weight(1f))
        if (trailing != null) Text(trailing, style = BentoType.label, color = Bento.colors.muted)
    }
}

@Composable
fun OneLine(text: String, style: TextStyle, color: Color, modifier: Modifier = Modifier) =
    Text(text, style = style, color = color, maxLines = 1, overflow = TextOverflow.Ellipsis, modifier = modifier)

/**
 * A single line that shrinks to fit the width it's given (down to [minSize]), so a title never
 * gets cut off or crowds the buttons beside it. Sizes are measured up front, so there's no flicker.
 */
@Composable
fun FitText(text: String, style: TextStyle, color: Color, modifier: Modifier = Modifier, minSize: TextUnit = 18.sp) {
    val measurer = rememberTextMeasurer()
    BoxWithConstraints(modifier) {
        val maxPx = constraints.maxWidth
        val size = remember(text, style, maxPx) {
            var s = style.fontSize.value
            while (s > minSize.value) {
                val w = measurer.measure(text, style.copy(fontSize = s.sp), maxLines = 1, softWrap = false).size.width
                if (w <= maxPx) break
                s -= 1f
            }
            s.sp
        }
        Text(
            text,
            style = style.copy(fontSize = size, lineHeight = size * 1.2f),
            color = color,
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

/** Square header button (search, add, themes). */
@Composable
fun HeaderButton(icon: ImageVector, label: String, selected: Boolean = false, onClick: () -> Unit) {
    val colors = Bento.colors
    val bg by animateColorAsState(if (selected) colors.ink else colors.card, Motion.colour(), label = "hbBg")
    val fg by animateColorAsState(if (selected) colors.bg else colors.ink, Motion.colour(), label = "hbFg")
    val interaction = remember { MutableInteractionSource() }
    Box(
        Modifier
            .size(44.dp)
            .pressScale(interaction, 0.92f)
            .clip(RoundedCornerShape(15.dp))
            .background(bg)
            .clickable(interactionSource = interaction, indication = null, role = Role.Button, onClickLabel = label, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(icon, contentDescription = label, tint = fg, modifier = Modifier.size(20.dp))
    }
}

/**
 * The app's menus. A small shadow plus a hairline border keeps the edge crisp; a large shadow
 * gets clipped by the menu's window, which is what made it look cut off.
 */
@Composable
fun BentoMenu(expanded: Boolean, onDismiss: () -> Unit, offset: DpOffset = DpOffset(8.dp, 4.dp), content: @Composable ColumnScope.() -> Unit) {
    val colors = Bento.colors
    DropdownMenu(
        expanded = expanded,
        onDismissRequest = onDismiss,
        offset = offset,
        shape = RoundedCornerShape(22.dp),
        containerColor = colors.sheet,
        tonalElevation = 0.dp,
        shadowElevation = 3.dp,
        border = BorderStroke(1.dp, colors.line),
        content = content,
    )
}

/** A short message with an optional action (usually Undo), shown at the bottom of the screen. */
@Composable
fun ToastPill(toast: Toast, onAction: () -> Unit) {
    val colors = Bento.colors
    Box(Modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Row(
            Modifier
                .clip(RoundedCornerShape(20.dp))
                .background(colors.ink)
                .padding(start = 18.dp, end = 6.dp, top = 6.dp, bottom = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(toast.message, style = BentoType.body, color = colors.bg, modifier = Modifier.padding(vertical = 8.dp))
            if (toast.actionLabel != null) {
                Spacer(Modifier.width(8.dp))
                Box(
                    Modifier.clip(RoundedCornerShape(14.dp)).background(colors.bg.copy(alpha = 0.16f))
                        .clickable(role = Role.Button, onClick = onAction).padding(horizontal = 14.dp, vertical = 8.dp)
                ) { Text(toast.actionLabel, style = BentoType.tag, color = colors.bg) }
            } else {
                Spacer(Modifier.width(12.dp))
            }
        }
    }
}
