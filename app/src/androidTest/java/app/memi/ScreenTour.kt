package app.memi

import android.graphics.Bitmap
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.test.SemanticsNodeInteraction
import androidx.compose.ui.test.junit4.ComposeTestRule
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.hasScrollToNodeAction
import androidx.compose.ui.test.hasSetTextAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.onLast
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
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

private fun capture(rule: ComposeTestRule, name: String) {
    // Compose tests run on a test clock: move it on so sheets, menus and fades finish animating,
    // then give the real display a moment to draw the result before taking the picture.
    rule.waitForIdle()
    rule.mainClock.advanceTimeBy(1500)
    rule.waitForIdle()
    InstrumentationRegistry.getInstrumentation().waitForIdleSync()
    Thread.sleep(600)
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
    private fun shot(name: String) = capture(rule, name)

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

    /** Finds a node, scrolling any list it may be hiding in (large text pushes things below the fold). */
    private fun find(m: SemanticsMatcher, last: Boolean = false): SemanticsNodeInteraction {
        if (rule.onAllNodes(m, useUnmergedTree = true).fetchSemanticsNodes().isEmpty()) {
            val lists = rule.onAllNodes(hasScrollToNodeAction()).fetchSemanticsNodes().size
            for (i in 0 until lists) {
                if (runCatching { rule.onAllNodes(hasScrollToNodeAction())[i].performScrollToNode(m) }.isSuccess) break
            }
        }
        val all = rule.onAllNodes(m, useUnmergedTree = true)
        val node = if (last) all.onLast() else all.onFirst()
        runCatching { node.performScrollTo() }
        rule.waitForIdle()
        return node
    }
    private fun text(t: String): SemanticsNodeInteraction = find(hasText(t, substring = true))
    /** A note inside the open sheet (the same words also appear on the category tile behind it). */
    private fun note(t: String): SemanticsNodeInteraction = find(hasText(t, substring = true), last = true)
    /** Maya in the people list: "Maya" on a phone, "Maya Okafor" where there's room for the full name. */
    private fun maya(): SemanticsNodeInteraction = find(hasText("Maya") or hasText("Maya Okafor"))
    private fun desc(d: String): SemanticsNodeInteraction = rule.onAllNodesWithContentDescription(d).onFirst()

    @Test fun t01_home() { text("Maya"); shot("01-home") }

    @Test fun t02_person() { maya().performClick(); shot("02-person") }

    @Test fun t03_category() {
        maya().performClick(); text("Gift ideas").performClick(); shot("03-category-sheet")
        note("blue ceramic mug").performTouchInput { longClick() }; shot("04-note-menu")
        text("Edit").performClick(); shot("05-note-editor")
    }

    @Test fun t04_newCategory() {
        maya().performClick(); text("New category").performClick(); shot("06-category-editor")
        text("A date").performClick(); shot("07-date-picker")
    }

    @Test fun t05_personMenu() {
        maya().performTouchInput { longClick() }; shot("08-person-menu")
        text("Edit").performClick(); shot("09-person-editor")
    }

    @Test fun t06_removePerson() {
        maya().performTouchInput { longClick() }; text("Remove").performClick(); shot("10-remove-dialog")
    }

    @Test fun t07_themesAndSync() {
        desc("Themes").performClick(); shot("11-theme-panel")
        text("Sync").performClick(); shot("12-sync-sheet")
    }

    @Test fun t08_search() {
        desc("Search").performClick(); rule.onAllNodes(hasSetTextAction()).onFirst().performTextInput("mug")
        shot("13-search")
    }

    @Test fun t09_toast() {
        maya().performClick(); text("Gift ideas").performClick()
        note("Signed copy").performTouchInput { longClick() }; find(hasText("Hit")).performClick(); shot("14-marked-hit")
    }
}

/** The photo cropper on its own, with a stand-in photo (the system picker can't be driven from a test). */
@RunWith(AndroidJUnit4::class)
class CropTour {
    @get:Rule val rule = createComposeRule()
    private fun shot(name: String) = capture(rule, name)

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
