@file:OptIn(ExperimentalMaterial3Api::class)

package app.memi.ui.sheets

import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.unit.IntRect
import app.memi.platform.rememberPhotoPicker
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AddAPhoto
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.memi.data.Dates
import app.memi.data.Note
import app.memi.data.NoteCollection
import app.memi.data.Person
import app.memi.ui.components.PersonPhoto
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import kotlinx.coroutines.launch
import java.time.Month
import java.time.format.TextStyle
import java.util.Locale

@Composable
fun BentoSheet(onDismiss: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
    val colors = Bento.colors
    app.memi.platform.DismissOnEscape(onDismiss)
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        containerColor = colors.sheet,
        contentColor = colors.ink,
        scrimColor = colors.scrim,
        shape = RoundedCornerShape(topStart = 32.dp, topEnd = 32.dp),
        dragHandle = {
            Box(Modifier.padding(top = 10.dp, bottom = 6.dp).width(40.dp).height(4.dp).clip(RoundedCornerShape(2.dp)).background(colors.handle))
        },
    ) {
        Column(
            Modifier
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 20.dp)
                .navigationBarsPadding()
                .imePadding(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            content = content,
        )
    }
}

@Composable
fun SheetTitle(text: String) = Text(text, style = BentoType.tileTitle, color = Bento.colors.ink, modifier = Modifier.padding(top = 4.dp))

@Composable
fun BentoField(
    label: String,
    value: String,
    onValueChange: (String) -> Unit,
    modifier: Modifier = Modifier,
    placeholder: String = "",
    singleLine: Boolean = true,
    keyboardType: KeyboardType = KeyboardType.Text,
    capitalization: KeyboardCapitalization = KeyboardCapitalization.Sentences,
) {
    val colors = Bento.colors
    Column(modifier) {
        Text(label, style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp, bottom = 6.dp))
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            singleLine = singleLine,
            minLines = if (singleLine) 1 else 3,
            textStyle = BentoType.input.copy(color = colors.ink),
            cursorBrush = SolidColor(colors.ink),
            keyboardOptions = KeyboardOptions(capitalization = capitalization, keyboardType = keyboardType),
            modifier = Modifier.fillMaxWidth(),
            decorationBox = { inner ->
                Box(
                    Modifier.fillMaxWidth().clip(RoundedCornerShape(16.dp)).background(colors.track).padding(horizontal = 16.dp, vertical = 14.dp)
                ) {
                    if (value.isEmpty() && placeholder.isNotEmpty()) Text(placeholder, style = BentoType.input, color = colors.muted)
                    inner()
                }
            },
        )
    }
}

@Composable
fun PrimaryButton(text: String, modifier: Modifier = Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    val colors = Bento.colors
    Box(
        modifier.height(54.dp).clip(RoundedCornerShape(18.dp))
            .background(if (enabled) colors.ink else colors.track)
            .clickable(enabled = enabled, role = Role.Button, onClick = onClick),
        contentAlignment = Alignment.Center,
    ) { Text(text, style = BentoType.cardTitle, color = if (enabled) colors.bg else colors.muted) }
}

@Composable
fun SecondaryButton(text: String, modifier: Modifier = Modifier, danger: Boolean = false, onClick: () -> Unit) {
    val colors = Bento.colors
    Box(
        modifier.height(54.dp).clip(RoundedCornerShape(18.dp)).background(colors.track)
            .clickable(role = Role.Button, onClick = onClick).padding(horizontal = 20.dp),
        contentAlignment = Alignment.Center,
    ) { Text(text, style = BentoType.cardTitle, color = if (danger) colors.danger else colors.ink) }
}

/** Photo loading and saving, provided by the view model. */
class PhotoTools(
    val loadFile: suspend (String) -> ImageBitmap?,
    /** Saves the part of the image inside [IntRect] as the profile photo; returns its path. */
    val saveCrop: suspend (ImageBitmap, IntRect) -> String?,
    val discard: (String) -> Unit,
)

/** Add or edit a person: photo (cropped in the app), name, relationship and birthday. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun PersonEditorSheet(
    initial: Person?,
    onDismiss: () -> Unit,
    photos: PhotoTools,
    onSave: (Person) -> Unit,
    onDelete: ((Person) -> Unit)?,
) {
    val colors = Bento.colors
    val scope = rememberCoroutineScope()
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var relationship by remember { mutableStateOf(initial?.relationship.orEmpty()) }
    var photo by remember { mutableStateOf(initial?.photoPath) }
    var day by remember { mutableStateOf(initial?.birthdayDay?.toString().orEmpty()) }
    var month by remember { mutableStateOf(initial?.birthdayMonth) }
    var year by remember { mutableStateOf(initial?.birthYear?.toString().orEmpty()) }
    var busy by remember { mutableStateOf(false) }
    var cropping by remember { mutableStateOf<ImageBitmap?>(null) }
    // Photos saved during this edit that aren't the one we end up keeping get cleaned up.
    val madeHere = remember { mutableListOf<String>() }

    val pickPhoto = rememberPhotoPicker(
        onLoading = { busy = it },
        onPicked = { image -> if (image != null) cropping = image },
    )
    fun recrop() {
        val path = photo ?: return
        scope.launch { busy = true; cropping = photos.loadFile(path); busy = false }
    }
    fun finish(person: Person?) {
        madeHere.filter { it != person?.photoPath }.forEach(photos.discard)
        if (person != null && initial?.photoPath != null && initial.photoPath != person.photoPath) photos.discard(initial.photoPath)
    }

    BentoSheet({ finish(null); onDismiss() }) {
        SheetTitle(if (initial == null) "New person" else "Edit ${initial.firstName}")
        Row(verticalAlignment = Alignment.CenterVertically) {
            Box(
                Modifier.size(width = 104.dp, height = 128.dp).clip(RoundedCornerShape(26.dp))
                    .clickable(role = Role.Button, onClickLabel = "Choose photo") {
                        pickPhoto()
                    },
            ) {
                PersonPhoto(
                    (initial ?: Person(name = name)).copy(photoPath = photo),
                    Modifier.size(width = 104.dp, height = 128.dp),
                    RoundedCornerShape(26.dp),
                )
                Box(
                    Modifier.align(Alignment.BottomEnd).padding(8.dp).size(34.dp).clip(RoundedCornerShape(12.dp)).background(colors.sheet),
                    contentAlignment = Alignment.Center,
                ) { Icon(Icons.Rounded.AddAPhoto, contentDescription = null, tint = colors.ink, modifier = Modifier.size(18.dp)) }
            }
            Spacer(Modifier.width(16.dp))
            Column(Modifier.weight(1f)) {
                Text(if (busy) "Opening photo…" else if (photo == null) "Add a photo" else "Change photo", style = BentoType.itemTitle, color = colors.ink)
                Text("You'll crop it next.", style = BentoType.caption, color = colors.muted)
                if (photo != null) {
                    Spacer(Modifier.height(8.dp))
                    Box(
                        Modifier.clip(RoundedCornerShape(12.dp)).background(colors.track)
                            .clickable(role = Role.Button) { recrop() }.padding(horizontal = 12.dp, vertical = 8.dp)
                    ) { Text("Adjust crop", style = BentoType.tag, color = colors.ink) }
                }
            }
        }
        BentoField("Name", name, { name = it }, placeholder = "Maya Okafor", capitalization = KeyboardCapitalization.Words)
        BentoField("Who they are to you", relationship, { relationship = it }, placeholder = "Sister, friend from uni…")

        Text("Birthday", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            BentoField("Day", day, { v -> day = v.filter(Char::isDigit).take(2) }, Modifier.width(84.dp), placeholder = "22", keyboardType = KeyboardType.Number)
            BentoField("Year (optional)", year, { v -> year = v.filter(Char::isDigit).take(4) }, Modifier.weight(1f), placeholder = "1995", keyboardType = KeyboardType.Number)
        }
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Month.entries.forEach { m ->
                val selected = month == m.value
                Box(
                    Modifier.clip(RoundedCornerShape(12.dp))
                        .background(if (selected) colors.ink else colors.track)
                        .clickable(role = Role.RadioButton) { month = if (selected) null else m.value }
                        .padding(horizontal = 12.dp, vertical = 9.dp)
                ) {
                    Text(m.getDisplayName(TextStyle.SHORT, Locale.getDefault()), style = BentoType.tag, color = if (selected) colors.bg else colors.ink)
                }
            }
        }

        Spacer(Modifier.height(4.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            if (initial != null && onDelete != null) SecondaryButton("Remove", danger = true) { finish(null); onDelete(initial) }
            PrimaryButton("Save", Modifier.weight(1f), enabled = name.isNotBlank()) {
                val d = day.toIntOrNull()?.takeIf { it in 1..31 }
                val hasBirthday = d != null && month != null
                val saved = (initial ?: Person(name = "")).copy(
                        name = name.trim(),
                        relationship = relationship.trim(),
                        photoPath = photo,
                        birthdayDay = if (hasBirthday) d else null,
                        birthdayMonth = if (hasBirthday) month else null,
                        birthYear = if (hasBirthday) year.toIntOrNull()?.takeIf { it in 1900..Dates.today().year } else null,
                    )
                finish(saved)
                onSave(saved)
            }
        }
    }

    cropping?.let { image ->
        CropDialog(
            image = image,
            onCancel = { cropping = null },
            onDone = { area ->
                cropping = null
                scope.launch {
                    busy = true
                    photos.saveCrop(image, area)?.let { path -> madeHere += path; photo = path }
                    busy = false
                }
            },
        )
    }

}

/** Edit a note's text, collection and deadline. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun NoteEditorSheet(
    note: Note,
    collections: List<NoteCollection>,
    onDismiss: () -> Unit,
    onSave: (Note) -> Unit,
) {
    val colors = Bento.colors
    var text by remember { mutableStateOf(note.text) }
    var collectionId by remember { mutableStateOf(note.collectionId) }
    var due by remember { mutableStateOf(note.dueAt?.let { Dates.toDate(it) }) }

    BentoSheet(onDismiss) {
        SheetTitle("Edit note")
        BentoField("Note", text, { text = it }, singleLine = false)
        Text("Collection", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            collections.forEach { c ->
                val selected = c.id == collectionId
                Box(
                    Modifier.clip(RoundedCornerShape(14.dp))
                        .background(if (selected) colors.ink else colors.tiles[c.slot % colors.tiles.size])
                        .clickable(role = Role.RadioButton) { collectionId = c.id }
                        .padding(horizontal = 14.dp, vertical = 10.dp)
                ) { Text(c.name, style = BentoType.tag, color = if (selected) colors.bg else colors.tileInks[c.slot % colors.tileInks.size]) }
            }
        }
        Text("Due", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        val today = Dates.today()
        val options = listOf<Pair<String, java.time.LocalDate?>>(
            "No date" to null,
            "Tomorrow" to today.plusDays(1),
            "This weekend" to Dates.nextWeekday(java.time.DayOfWeek.SATURDAY, today, allowToday = true),
            "Next week" to today.plusDays(7),
            "In a month" to today.plusMonths(1),
        )
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            options.forEach { (label, date) ->
                val selected = due == date
                Box(
                    Modifier.clip(RoundedCornerShape(14.dp))
                        .background(if (selected) colors.ink else colors.track)
                        .clickable(role = Role.RadioButton) { due = date }
                        .padding(horizontal = 14.dp, vertical = 10.dp)
                ) { Text(label, style = BentoType.tag, color = if (selected) colors.bg else colors.ink) }
            }
            val current = due
            if (current != null && options.none { it.second == current }) {
                Box(
                    Modifier.clip(RoundedCornerShape(14.dp)).background(colors.ink).padding(horizontal = 14.dp, vertical = 10.dp)
                ) { Text(Dates.shortDate(current), style = BentoType.tag, color = colors.bg) }
            }
        }
        Spacer(Modifier.height(4.dp))
        PrimaryButton("Save", Modifier.fillMaxWidth(), enabled = text.isNotBlank()) {
            onSave(note.copy(text = text.trim(), collectionId = collectionId, dueAt = due?.let { Dates.endOfDay(it) }))
        }
    }
}

/** Name a new collection, or rename one. */
@Composable
fun NameDialog(title: String, initial: String, confirm: String, onDismiss: () -> Unit, onConfirm: (String) -> Unit) {
    val colors = Bento.colors
    var value by remember { mutableStateOf(initial) }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = colors.sheet,
        title = { Text(title, style = BentoType.section, color = colors.ink) },
        text = { BentoField("Name", value, { value = it }, placeholder = "Books, Recipes, Date nights…", capitalization = KeyboardCapitalization.Words) },
        confirmButton = {
            TextButton(enabled = value.isNotBlank(), onClick = { onConfirm(value.trim()) }) { Text(confirm, style = BentoType.itemTitle, color = colors.ink) }
        },
        dismissButton = { TextButton(onClick = onDismiss) { Text("Cancel", style = BentoType.itemTitle, color = colors.muted) } },
    )
}
