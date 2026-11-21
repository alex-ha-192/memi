package app.memi.sync

import android.content.Context
import android.net.Uri
import android.provider.DocumentsContract
import app.memi.engine.HostFile
import app.memi.engine.HostFolder

/**
 * A folder the person picked with the system picker (Syncthing's, Dropbox's, a USB drive...),
 * which sync reads and writes Memi's encrypted files in. Memi only touches its own files there.
 * The picker's permission is kept across restarts (takePersistableUriPermission, done when picking).
 */
class SafFolder(private val context: Context, private val tree: Uri) : HostFolder {
    private val resolver get() = context.contentResolver
    private val root: Uri get() = DocumentsContract.buildDocumentUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))

    private data class Doc(val id: String, val name: String, val modified: Long, val size: Long, val folder: Boolean)

    private fun children(): List<Doc>? = runCatching {
        val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, DocumentsContract.getTreeDocumentId(tree))
        val cols = arrayOf(
            DocumentsContract.Document.COLUMN_DOCUMENT_ID, DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED, DocumentsContract.Document.COLUMN_SIZE,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
        )
        resolver.query(uri, cols, null, null, null)?.use { c ->
            buildList {
                while (c.moveToNext()) {
                    add(Doc(
                        id = c.getString(0), name = c.getString(1) ?: "",
                        modified = if (c.isNull(2)) 0L else c.getLong(2), size = if (c.isNull(3)) 0L else c.getLong(3),
                        folder = c.getString(4) == DocumentsContract.Document.MIME_TYPE_DIR,
                    ))
                }
            }
        }
    }.getOrNull()

    private fun find(name: String): Uri? =
        children()?.firstOrNull { !it.folder && it.name == name }?.let { DocumentsContract.buildDocumentUriUsingTree(tree, it.id) }

    override fun list(): List<HostFile>? = children()?.filter { !it.folder }?.map { HostFile(it.name, it.modified, it.size) }

    override fun read(name: String): ByteArray? = runCatching {
        find(name)?.let { uri -> resolver.openInputStream(uri)?.use { it.readBytes() } }
    }.getOrNull()

    override fun write(name: String, bytes: ByteArray): String? = runCatching {
        val uri = find(name)
            ?: DocumentsContract.createDocument(resolver, root, "application/octet-stream", name)
            ?: error("Couldn't make $name")
        resolver.openOutputStream(uri, "wt")?.use { it.write(bytes) } ?: error("Couldn't open $name")
        Unit
    }.exceptionOrNull()?.let { it.message ?: it.toString() }

    override fun delete(name: String): String? = runCatching {
        find(name)?.let { DocumentsContract.deleteDocument(resolver, it) }
        Unit
    }.exceptionOrNull()?.let { it.message ?: it.toString() }

    override fun label(): String = runCatching {
        resolver.query(root, arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }
    }.getOrNull() ?: "your folder"
}
