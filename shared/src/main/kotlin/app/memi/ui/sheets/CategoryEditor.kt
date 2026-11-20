@file:OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)

package app.memi.ui.sheets

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.unit.dp
import app.memi.data.Dates
import app.memi.data.NoteCollection
import app.memi.data.Occasion
import app.memi.data.Person
import app.memi.ui.theme.Bento
import app.memi.ui.theme.BentoType
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneOffset

/**
 * Create or edit one of a person's categories. A category can build up to their birthday or a
 * date (once, like a trip, or every year, like an anniversary); anything in it then counts as
 * due by that day and shows in Still fresh as it gets close.
 */
@Composable
fun CategoryEditorSheet(
    person: Person,
    initial: NoteCollection?,
    onDismiss: () -> Unit,
    onSave: (name: String, occasion: String, occasionDay: Long?, yearly: Boolean) -> Unit,
) {
    val colors = Bento.colors
    var name by remember { mutableStateOf(initial?.name.orEmpty()) }
    var occasion by remember { mutableStateOf(initial?.occasion ?: Occasion.NONE) }
    var day by remember { mutableStateOf(initial?.occasionDay) }
    var yearly by remember { mutableStateOf(initial?.yearly ?: false) }
    var picking by remember { mutableStateOf(false) }

    BentoSheet(onDismiss) {
        SheetTitle(if (initial == null) "New category for ${person.firstName}" else "Edit ${initial.name}")
        BentoField("Name", name, { name = it }, placeholder = "Anniversary, Trip to Lisbon, Books…", capitalization = KeyboardCapitalization.Words)

        Text("Builds up to", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Choice("Nothing", occasion == Occasion.NONE) { occasion = Occasion.NONE }
            Choice("${person.firstName}'s birthday", occasion == Occasion.BIRTHDAY) { occasion = Occasion.BIRTHDAY }
            Choice("A date", occasion == Occasion.DATE) { occasion = Occasion.DATE; if (day == null) picking = true }
        }
        if (occasion == Occasion.BIRTHDAY && !person.hasBirthday) {
            Text("${person.firstName} doesn't have a birthday saved yet. Add one from their profile.", style = BentoType.caption, color = colors.muted, modifier = Modifier.padding(start = 4.dp))
        }
        if (occasion == Occasion.DATE) {
            FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                val d = day?.let { LocalDate.ofEpochDay(it) }
                Choice(d?.let { "${Dates.shortDate(it)} ${it.year}" } ?: "Pick a date", selected = d != null) { picking = true }
                Choice("Just once", !yearly) { yearly = false }
                Choice("Every year", yearly) { yearly = true }
            }
        }

        Spacer(Modifier.height(4.dp))
        val valid = name.isNotBlank() && (occasion != Occasion.DATE || day != null)
        PrimaryButton("Save", Modifier.fillMaxWidth(), enabled = valid) {
            onSave(name.trim(), occasion, if (occasion == Occasion.DATE) day else null, occasion == Occasion.DATE && yearly)
        }
    }

    if (picking) {
        val state = rememberDatePickerState(initialSelectedDateMillis = (day ?: Dates.today().toEpochDay()) * 86_400_000L)
        DatePickerDialog(
            onDismissRequest = { picking = false },
            confirmButton = {
                TextButton(onClick = {
                    state.selectedDateMillis?.let { ms -> day = Instant.ofEpochMilli(ms).atZone(ZoneOffset.UTC).toLocalDate().toEpochDay() }
                    picking = false
                }) { Text("Done", style = BentoType.itemTitle, color = colors.ink) }
            },
            dismissButton = { TextButton(onClick = { picking = false }) { Text("Cancel", style = BentoType.itemTitle, color = colors.muted) } },
            colors = DatePickerDefaults.colors(containerColor = colors.sheet),
        ) {
            DatePicker(state = state, colors = DatePickerDefaults.colors(containerColor = colors.sheet))
        }
    }
}

@Composable
private fun Choice(label: String, selected: Boolean, onClick: () -> Unit) {
    val colors = Bento.colors
    Box(
        Modifier.clip(RoundedCornerShape(14.dp))
            .background(if (selected) colors.ink else colors.track)
            .clickable(role = Role.RadioButton, onClick = onClick)
            .padding(horizontal = 14.dp, vertical = 10.dp)
    ) { Text(label, style = BentoType.tag, color = if (selected) colors.bg else colors.ink) }
}
