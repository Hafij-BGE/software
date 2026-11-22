package com.hafij.inkwell;

import android.content.ContentResolver;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileNotFoundException;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;
import java.util.List;

/** Storage Access Framework helpers for the library folder the user picked. */
public final class SafDocs {
    private SafDocs() {
    }

    /** One file or folder inside the library tree. */
    public static final class Entry {
        public final Uri uri;
        public final String docId;
        public final String name;
        public final String mime;
        public final long modified;
        public final long size;

        Entry(Uri uri, String docId, String name, String mime, long modified, long size) {
            this.uri = uri;
            this.docId = docId;
            this.name = name;
            this.mime = mime;
            this.modified = modified;
            this.size = size;
        }

        public boolean isDir() {
            return Document.MIME_TYPE_DIR.equals(mime);
        }
    }

    private static final String[] COLS = {Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE, Document.COLUMN_LAST_MODIFIED, Document.COLUMN_SIZE};

    public static String rootId(Uri tree) {
        return DocumentsContract.getTreeDocumentId(tree);
    }

    public static Uri docUri(Uri tree, String docId) {
        return DocumentsContract.buildDocumentUriUsingTree(tree, docId);
    }

    public static List<Entry> list(ContentResolver cr, Uri tree, String parentId) {
        List<Entry> out = new ArrayList<>();
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parentId);
        try (Cursor c = cr.query(children, COLS, null, null, null)) {
            if (c == null) return out;
            while (c.moveToNext()) {
                String id = c.getString(0);
                out.add(new Entry(docUri(tree, id), id, c.getString(1), c.getString(2),
                        c.isNull(3) ? 0 : c.getLong(3), c.isNull(4) ? 0 : c.getLong(4)));
            }
        } catch (RuntimeException ignored) {
            // Folder vanished or the grant was revoked.
        }
        return out;
    }

    public static Entry child(ContentResolver cr, Uri tree, String parentId, String name) {
        for (Entry e : list(cr, tree, parentId)) if (e.name.equals(name)) return e;
        return null;
    }

    /** Creates a document; returns its uri. Fails if the provider renames it. */
    public static Uri create(ContentResolver cr, Uri tree, String parentId, String mime, String name) throws IOException {
        try {
            Uri uri = DocumentsContract.createDocument(cr, docUri(tree, parentId), mime, name);
            if (uri == null) throw new IOException("Could not create " + name);
            return uri;
        } catch (FileNotFoundException | RuntimeException e) {
            throw new IOException("Could not create " + name + ": " + e.getMessage(), e);
        }
    }

    /** A name not used yet in the folder: "base.ext", "base (2).ext", ... */
    public static String uniqueName(ContentResolver cr, Uri tree, String parentId, String base, String ext) {
        List<String> names = new ArrayList<>();
        for (Entry e : list(cr, tree, parentId)) names.add(e.name.toLowerCase());
        String n = base + ext;
        for (int i = 2; names.contains(n.toLowerCase()); i++) n = base + " (" + i + ")" + ext;
        return n;
    }

    public static Uri rename(ContentResolver cr, Uri doc, String name) throws IOException {
        try {
            Uri u = DocumentsContract.renameDocument(cr, doc, name);
            return u == null ? doc : u;
        } catch (FileNotFoundException | RuntimeException e) {
            throw new IOException("Could not rename: " + e.getMessage(), e);
        }
    }

    public static void delete(ContentResolver cr, Uri doc) {
        try {
            DocumentsContract.deleteDocument(cr, doc);
        } catch (FileNotFoundException | RuntimeException ignored) {
            // Already gone.
        }
    }

    public static void copy(ContentResolver cr, Uri from, File to) throws IOException {
        try (InputStream in = cr.openInputStream(from); OutputStream out = new FileOutputStream(to)) {
            if (in == null) throw new IOException("Cannot read " + from);
            pipe(in, out);
        }
    }

    public static void copy(ContentResolver cr, File from, Uri to) throws IOException {
        try (InputStream in = new FileInputStream(from); OutputStream out = cr.openOutputStream(to, "wt")) {
            if (out == null) throw new IOException("Cannot write " + to);
            pipe(in, out);
        }
    }

    public static void copy(ContentResolver cr, Uri from, Uri to) throws IOException {
        try (InputStream in = cr.openInputStream(from); OutputStream out = cr.openOutputStream(to, "wt")) {
            if (in == null || out == null) throw new IOException("Cannot copy");
            pipe(in, out);
        }
    }

    public static void write(ContentResolver cr, Uri to, byte[] data) throws IOException {
        try (OutputStream out = cr.openOutputStream(to, "wt")) {
            if (out == null) throw new IOException("Cannot write " + to);
            out.write(data);
        }
    }

    private static void pipe(InputStream in, OutputStream out) throws IOException {
        byte[] buf = new byte[1 << 16];
        int n;
        while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
    }
}
