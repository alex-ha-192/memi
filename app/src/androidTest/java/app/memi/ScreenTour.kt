package app.memi

import android.graphics.Bitmap
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.longClick
import androidx.compose.ui.test.onAllNodesWithContentDescription
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.memi.data.Person
import app.memi.data.ThemeChoice
import app.memi.ui.sheets.CropDialog
import app.memi.ui.theme.BentoTheme
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import org.junit.Before
import org.junit.FixMethodOrder
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import java.io.File
import java.time.LocalDate
import java.time.ZoneId

/**
 * Not a pass/fail test: it walks through every screen, sheet, menu and dialog and saves a
 * screenshot of each (dialogs included, since it captures the whole display), so layouts can be
 * checked by eye on a real phone or tablet. CI runs it in several passes (light, dark with large
 * text, tablet) and uploads the pictures. The pass name comes from the "pass" runner argument.
 */
private val pass: String get() = InstrumentationRegistry.getArguments().getString("pass") ?: "phone"

private fun shot(name: String) {
    InstrumentationRegistry.getInstrumentation().waitForIdleSync()
    Thread.sleep(900) // let sheets and fades settle
    val bmp = InstrumentationRegistry.getInstrumentation().uiAutomation.takeScreenshot() ?: return
    val app = ApplicationProvider.getApplicationContext<MemiApp>()
    val dir = File(app.filesDir, "shots").apply { mkdirs() }
    File(dir, "$pass-$name.png").outputStream().use { bmp.compress(Bitmap.CompressFormat.PNG, 100, it) }
}

@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class ScreenTour {
    @get:Rule val rule = createAndroidComposeRule<MainActivity>()
    private val app get() = ApplicationProvider.getApplicationContext<MemiApp>()

    @Before
    fun seed() = runBlocking {
        val repo = app.repository
        if (repo.people.first().none { it.name == "Maya Okafor" }) {
            val soon = LocalDate.now().plusDays(6)
            val maya = repo.addPerson(Person(name = "Maya Okafor", relationship = "Sister", birthdayMonth = soon.monthValue, birthdayDay = soon.dayOfMonth, birthYear = 1994))
            val sam = repo.addPerson(Person(name = "Sam Lee", relationship = "Friend from uni"))
            repo.addPerson(Person(name = "Priya Raman", relationship = "Neighbour"))
            val cats = repo.collections.first()
            fun cat(person: Long, slot: Int) = cats.filter { it.personId == person }.sortedBy { it.sortOrder }[slot].id
            repo.addNote(maya, cat(maya, 0), "The blue ceramic mug from the market in Porto")
            repo.addNote(maya, cat(maya, 0), "Signed copy of the new Sally Rooney novel")
            repo.addNote(maya, cat(maya, 1), "Pottery class together in the spring", LocalDate.now().plusDays(3).atStartOfDay(ZoneId.systemDefault()).toInstant().toEpochMilli())
            repo.addNote(maya, cat(maya, 2), "Allergic to hazelnuts, so no praline")
            repo.addNote(sam, cat(sam, 2), "Takes his coffee black with one sugar")
        }
        rule.waitForIdle()
    }

    private fun text(t: String): SemanticsNodeInteraction = rule.onAllNodesWithText(t, substring = true, useUnmergedTree = true).onFirst()
    private fun desc(d: String): SemanticsNodeInteraction = rule.onAllNodesWithContentDescription(d).onFirst()

    @Test fun t01_home() { text("Maya"); shot("01-home") }

    @Test fun t02_person() { text("Maya").performClick(); shot("02-person") }

    @Test fun t03_category() {
        text("Maya").performClick(); text("Gift ideas").performClick(); shot("03-category-sheet")
        text("blue ceramic mug").performTouchInput { longClick() }; shot("04-note-menu")
        text("Edit").performClick(); shot("05-note-editor")
    }

    @Test fun t04_newCategory() {
        text("Maya").performClick(); text("New category").performClick(); shot("06-category-editor")
        text("A date").performClick(); shot("07-date-picker")
    }

    @Test fun t05_personMenu() {
        text("Maya").performTouchInput { longClick() }; shot("08-person-menu")
        text("Edit").performClick(); shot("09-person-editor")
    }

    @Test fun t06_removePerson() {
        text("Maya").performTouchInput { longClick() }; text("Remove").performClick(); shot("10-remove-dialog")
    }

    @Test fun t07_themesAndSync() {
        desc("Themes").performClick(); shot("11-theme-panel")
        text("Sync").performClick(); shot("12-sync-sheet")
    }

    @Test fun t08_search() {
        desc("Search").performClick(); rule.onAllNodesWithText("Search people and notes", useUnmergedTree = true).onFirst().performTextInput("mug")
        shot("13-search")
    }

    @Test fun t09_toast() {
        text("Maya").performClick(); text("Gift ideas").performClick()
        text("Signed copy").performTouchInput { longClick() }; text("hit").performClick(); shot("14-marked-hit")
    }
}

/** The photo cropper on its own, with a stand-in photo (the system picker can't be driven from a test). */
@RunWith(AndroidJUnit4::class)
class CropTour {
    @get:Rule val rule = createComposeRule()

    private fun samplePhoto(): ImageBitmap {
        val bmp = Bitmap.createBitmap(1600, 1200, Bitmap.Config.ARGB_8888)
        val c = android.graphics.Canvas(bmp)
        c.drawColor(0xFFCFE3F0.toInt())
        val p = android.graphics.Paint().apply { isAntiAlias = true }
        p.color = 0xFFF2B999.toInt(); c.drawCircle(800f, 520f, 300f, p)
        p.color = 0xFF6C8EA4.toInt(); c.drawRect(0f, 900f, 1600f, 1200f, p)
        return bmp.asImageBitmap()
    }

    @Test fun crop() {
        val photo = samplePhoto()
        rule.setContent { BentoTheme(ThemeChoice()) { Box(Modifier.fillMaxSize()) { CropDialog(photo, onCancel = {}, onDone = {}) } } }
        shot("15-crop")
    }
}
