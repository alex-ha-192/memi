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

/** A folder kept by Kotlin, the way the app's picked folder is. */
class TestFolder(private val dir: File) : HostFolder {
    override fun list(): List<HostFile>? = dir.listFiles()?.filter { it.isFile }?.map { HostFile(it.name, it.lastModified(), it.length()) }
    override fun read(name: String): ByteArray? = File(dir, name).takeIf { it.isFile }?.readBytes()
    override fun write(name: String, bytes: ByteArray): String? = runCatching { File(dir, name).writeBytes(bytes) }.exceptionOrNull()?.message
    override fun delete(name: String): String? { File(dir, name).delete(); return null }
    override fun label() = "Sync folder"
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
    // Sync through a folder Kotlin keeps: this phone turns it on, a second one joins with the
    // recovery key, and an edit on either reaches the other.
    val shared = File(dir, "shared").apply { mkdirs() }
    e.connectFolder("content://test/tree/shared", TestFolder(shared))
    check(e.status().sync.enabled && e.provider() == SyncProvider.Folder("content://test/tree/shared"), "sync on through a folder")
    check(File(shared, "memi-backup.bin").length() > 0, "the encrypted copy is in the folder")
    check(e.status().sync.account == "Sync folder", "Sync names the folder")
    val dir2 = File(dir, "second").apply { mkdirs() }
    val host2 = TestHost()
    val e2 = Engine(File(dir2, "memi.db").path, File(dir2, "copy.db").path, File(dir2, "photos").path, File(dir2, "sync.json").path, null, host2)
    e2.setHostFolder(TestFolder(shared))
    e2.connectFolder("content://test/tree/shared", TestFolder(shared))
    check(e2.status().problem is Problem.NeedsKey, "the second phone asks for the key")
    check(e2.useRecoveryKey("", e.recoveryKey()!!), "and the recovery key opens it")
    check(e2.everything().people.map { it.name }.sorted() == listOf("Maya Okafor", "Sam Lee"), "everyone arrives")
    val n2 = e2.addNote(e2.everything().people.first { it.name == "Sam Lee" }.id, e2.everything().collections.first().id, "Board games", null)
    e2.syncNow("")
    e.syncNow("")
    check(e.everything().notes.any { it.text == "Board games" }, "an edit on the second phone reaches the first ($n2)")
    e.setWebdavPassword(null)
    try { e.connectWebdav("http://example.com/dav", "me", "pw"); error("should throw") } catch (x: MemiException) { check(x.message!!.contains("https"), "plain http to the internet is refused") }
    e2.close()

    dir.deleteRecursively()
    println("ALL GOOD")
}
