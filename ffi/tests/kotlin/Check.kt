// Runs memi-engine's real Kotlin bindings against the library built for this computer, the way
// the Android app uses them: edits, ids, undo, ordering, errors, reopening, and the first open of
// an existing database. Run with ./run.sh.

import app.memi.engine.*
import java.io.File

class TestHost : Host {
    var stored: ByteArray? = null
    override fun key(): ByteArray? = stored
    override fun saveKey(key: ByteArray) { stored = key }
    override fun forgetKey() { stored = null }
    override fun deviceLabel() = "Test phone"
}

fun check(ok: Boolean, what: String) { if (!ok) error("FAILED: $what"); println("ok  $what") }

fun main() {
    val dir = File(System.getProperty("java.io.tmpdir"), "memi-jvm-" + System.nanoTime()).apply { mkdirs() }
    val db = File(dir, "memi.db").path
    val host = TestHost()
    val legacy = SyncState(enabled = true, account = "me@example.com", remoteVersion = "v9", dirty = true, lastSync = 1234L)
    val copy = File(dir, "memi-0.6.0.db")
    var e = Engine(db, copy.path, File(dir, "photos").path, File(dir, "sync.json").path, legacy, host)
    check(!copy.exists(), "no copy of a database that didn't exist")
    check(e.status().sync.account == "me@example.com", "old sync settings carried over")
    check(File(dir, "sync.json").exists(), "and saved to sync.json")

    val maya = e.addPerson(Person(0, "", 0, "Maya Okafor", "Sister", null, 11, 26, 1994, 0, false, 0, 0))
    val all = e.everything()
    check(all.people.single().birthdayMonth == 11, "person added with birthday")
    check(all.collections.count { it.personId == maya } == 3, "three starter categories")
    val gifts = all.collections.first { it.personId == maya && it.kind == "GIFT" }
    check(gifts.occasion == "BIRTHDAY", "gift ideas build up to the birthday")
    val n = e.addNote(maya, gifts.id, "  Blue mug  ", null)
    val note = e.everything().notes.single { it.id == n }
    check(note.text == "Blue mug", "note text trimmed")
    e.markDone(n, 1)
    check(e.everything().notes.single().status == 1, "marked done")
    e.reopen(n)
    val reopened = e.everything().notes.single()
    e.deleteNote(n)
    check(e.everything().notes.isEmpty(), "deleted")
    e.restoreNote(reopened)
    check(e.everything().notes.single().uid == note.uid, "undo brings back the same note")
    val v = e.version()
    e.setPinned(maya, true)
    check(e.version() > v && e.everything().people.single().pinned, "pinned, version moved on")
    val sam = e.addPerson(Person(0, "", 0, "Sam Lee", "Friend", null, null, null, null, 0, false, 0, 0))
    e.setPinned(maya, false)
    e.placePerson(sam, maya)
    check(e.everything().people.map { it.name } == listOf("Maya Okafor", "Sam Lee"), "drag Sam onto Maya below him: Sam goes after her")
    val c = e.addCollection(maya, "Anniversary", "DATE", 20000L, true)
    e.moveCollection(c, -1)
    val mine = e.everything().collections.filter { it.personId == maya }.sortedBy { it.sortOrder }.map { it.name }
    check(mine == listOf("Gift ideas", "Plans", "Anniversary", "Notes"), "category moved left: $mine")
    e.deleteCollection(gifts.id)
    check(e.everything().notes.single().collectionId != gifts.id, "notes kept when a category goes")
    try { e.deleteNote(99999); error("should throw") } catch (x: MemiException) { check(x.message!!.contains("no longer here"), "missing id is an error, not a crash") }
    check(e.recoveryKey() == null, "no key yet")
    e.turnOff(false)
    check(!e.status().sync.enabled, "sync off")

    // Reopen: everything is on disk.
    e.close()
    val copy2 = File(dir, "memi-before.db")
    e = Engine(db, copy2.path, File(dir, "photos").path, File(dir, "sync.json").path, null, host)
    check(copy2.length() > 0, "an existing database is copied before it's opened")
    val again = e.everything()
    check(again.people.size == 2 && again.notes.size == 1 && again.collections.size == 6, "all there after reopening")
    check(again.people.first { it.name == "Maya Okafor" }.id == maya, "ids stay the same")
    dir.deleteRecursively()
    println("ALL GOOD")
}
