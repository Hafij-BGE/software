package io.github.hafijbge.markd;

import android.content.ContentResolver;
import android.content.Intent;
import android.content.UriPermission;
import android.database.Cursor;
import android.net.Uri;
import android.provider.DocumentsContract;
import android.provider.OpenableColumns;

import java.io.ByteArrayOutputStream;
import java.io.FileNotFoundException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.URLDecoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

/**
 * Storage Access Framework helpers. A document opened on its own grants access
 * to that file only; images next to it are reachable through a persisted tree
 * (folder) grant that contains the document.
 */
final class DocStore {

    /** A writable folder: the tree grant it was reached through and its document id. */
    static final class Folder {
        final Uri tree;
        final String dirId;

        Folder(Uri tree, String dirId) {
            this.tree = tree;
            this.dirId = dirId;
        }

        Uri uri() {
            return DocumentsContract.buildDocumentUriUsingTree(tree, dirId);
        }
    }

    static final class Entry {
        final String docId;
        final String name;
        final String mime;
        final long modified;

        Entry(String docId, String name, String mime, long modified) {
            this.docId = docId;
            this.name = name;
            this.mime = mime;
            this.modified = modified;
        }

        boolean isDir() {
            return DocumentsContract.Document.MIME_TYPE_DIR.equals(mime);
        }
    }

    private final ContentResolver resolver;

    DocStore(ContentResolver resolver) {
        this.resolver = resolver;
    }

    void persist(Uri uri, int flags) {
        int wanted = flags & (Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        try {
            resolver.takePersistableUriPermission(uri, wanted);
        } catch (SecurityException e) {
            try {
                resolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (SecurityException ignored) {
                // Provider does not offer persistable grants; access lasts for this session.
            }
        }
    }

    String displayName(Uri uri) {
        try (Cursor c = resolver.query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (c != null && c.moveToFirst() && !c.isNull(0)) return c.getString(0);
        } catch (RuntimeException ignored) {
            // Fall through to the path segment.
        }
        String last = uri.getLastPathSegment();
        if (last == null) return "Untitled.md";
        int slash = last.lastIndexOf('/');
        return slash >= 0 ? last.substring(slash + 1) : last;
    }

    byte[] readBytes(Uri uri) throws IOException {
        try (InputStream in = resolver.openInputStream(uri)) {
            if (in == null) throw new FileNotFoundException(uri.toString());
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buffer = new byte[64 * 1024];
            int n;
            while ((n = in.read(buffer)) > 0) out.write(buffer, 0, n);
            return out.toByteArray();
        }
    }

    String readText(Uri uri) throws IOException {
        String text = new String(readBytes(uri), StandardCharsets.UTF_8);
        return text.startsWith("﻿") ? text.substring(1) : text;
    }

    void writeBytes(Uri uri, byte[] data) throws IOException {
        OutputStream out;
        try {
            // "wt" truncates; some providers only implement "w".
            out = resolver.openOutputStream(uri, "wt");
        } catch (IllegalArgumentException | UnsupportedOperationException | FileNotFoundException e) {
            out = resolver.openOutputStream(uri, "w");
        }
        if (out == null) throw new FileNotFoundException(uri.toString());
        try (OutputStream stream = out) {
            stream.write(data);
            stream.flush();
        }
    }

    List<Entry> list(Uri tree, String dirId) {
        List<Entry> entries = new ArrayList<>();
        Uri children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, dirId);
        String[] columns = {
            DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME,
            DocumentsContract.Document.COLUMN_MIME_TYPE,
            DocumentsContract.Document.COLUMN_LAST_MODIFIED
        };
        try (Cursor c = resolver.query(children, columns, null, null, null)) {
            while (c != null && c.moveToNext()) {
                entries.add(new Entry(c.getString(0), c.getString(1), c.getString(2), c.isNull(3) ? 0 : c.getLong(3)));
            }
        }
        return entries;
    }

    /** Folder containing {@code doc} that a persisted tree grant lets us write, or null. */
    Folder folderOf(Uri doc) {
        if (doc == null) return null;
        String docId;
        try {
            docId = DocumentsContract.getDocumentId(doc);
        } catch (IllegalArgumentException e) {
            return null;
        }
        String authority = doc.getAuthority();
        for (UriPermission permission : resolver.getPersistedUriPermissions()) {
            Uri tree = permission.getUri();
            if (!permission.isWritePermission() || !DocumentsContract.isTreeUri(tree)) continue;
            if (authority == null || !authority.equals(tree.getAuthority())) continue;
            String parent = parentWithin(tree, docId);
            if (parent != null) return new Folder(tree, parent);
        }
        return null;
    }

    /** Document id of the parent of {@code docId}, if it lies inside {@code tree}. */
    private String parentWithin(Uri tree, String docId) {
        String treeId = DocumentsContract.getTreeDocumentId(tree);
        try {
            Uri inTree = DocumentsContract.buildDocumentUriUsingTree(tree, docId);
            DocumentsContract.Path path = DocumentsContract.findDocumentPath(resolver, inTree);
            if (path != null) {
                List<String> ids = path.getPath();
                if (ids.size() >= 2) return ids.get(ids.size() - 2);
            }
        } catch (RuntimeException | FileNotFoundException ignored) {
            // Not inside this tree, or the provider has no findDocumentPath.
        }
        // Path-style ids (ExternalStorageProvider): "primary:Documents/notes/a.md".
        String parent = pathParent(docId);
        if (parent == null) return null;
        String prefix = treeId.endsWith("/") || treeId.endsWith(":") ? treeId : treeId + "/";
        return parent.equals(treeId) || parent.startsWith(prefix) ? parent : null;
    }

    private static String pathParent(String docId) {
        int slash = docId.lastIndexOf('/');
        if (slash >= 0) return docId.substring(0, slash);
        int colon = docId.indexOf(':');
        return colon >= 0 && colon < docId.length() - 1 ? docId.substring(0, colon + 1) : null;
    }

    private String findChild(Uri tree, String dirId, String name) {
        for (Entry entry : list(tree, dirId)) {
            if (entry.name.equals(name)) return entry.docId;
        }
        return null;
    }

    private static List<String> segments(String relativePath) {
        List<String> parts = new ArrayList<>();
        for (String raw : relativePath.split("/")) {
            if (raw.isEmpty() || raw.equals(".")) continue;
            try {
                parts.add(URLDecoder.decode(raw.replace("+", "%2B"), "UTF-8"));
            } catch (Exception e) {
                parts.add(raw);
            }
        }
        return parts;
    }

    /** Document uri of {@code relativePath} under {@code folder}, or null. Supports ".." for path-style ids. */
    Uri resolve(Folder folder, String relativePath) {
        String current = folder.dirId;
        String treeId = DocumentsContract.getTreeDocumentId(folder.tree);
        for (String part : segments(relativePath)) {
            if (part.equals("..")) {
                if (current.equals(treeId)) return null;
                current = pathParent(current);
                if (current == null) return null;
                continue;
            }
            current = findChild(folder.tree, current, part);
            if (current == null) return null;
        }
        return DocumentsContract.buildDocumentUriUsingTree(folder.tree, current);
    }

    /**
     * Create {@code relativePath} (e.g. "assets/sketch-1.png") under {@code folder},
     * making directories as needed. Returns the path actually written, which
     * differs when the provider renamed the file to avoid a clash.
     */
    String createFile(Folder folder, String relativePath, String mime, byte[] data) throws IOException {
        List<String> parts = segments(relativePath);
        if (parts.isEmpty() || parts.contains("..")) throw new IOException("Invalid path " + relativePath);
        String dirId = folder.dirId;
        StringBuilder written = new StringBuilder();
        for (int i = 0; i < parts.size() - 1; i++) {
            String name = parts.get(i);
            String child = findChild(folder.tree, dirId, name);
            if (child == null) {
                Uri dir = DocumentsContract.createDocument(resolver,
                    DocumentsContract.buildDocumentUriUsingTree(folder.tree, dirId),
                    DocumentsContract.Document.MIME_TYPE_DIR, name);
                if (dir == null) throw new IOException("Cannot create folder " + name);
                child = DocumentsContract.getDocumentId(dir);
                name = displayName(dir);
            }
            dirId = child;
            written.append(name).append('/');
        }
        Uri file = DocumentsContract.createDocument(resolver,
            DocumentsContract.buildDocumentUriUsingTree(folder.tree, dirId), mime, parts.get(parts.size() - 1));
        if (file == null) throw new IOException("Cannot create " + relativePath);
        writeBytes(file, data);
        written.append(displayName(file));
        return written.toString();
    }

    Uri createInFolder(Uri tree, String dirId, String mime, String name) throws FileNotFoundException {
        return DocumentsContract.createDocument(resolver, DocumentsContract.buildDocumentUriUsingTree(tree, dirId), mime, name);
    }
}
