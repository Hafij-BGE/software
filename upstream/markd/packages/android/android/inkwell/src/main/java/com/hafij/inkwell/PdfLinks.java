package com.hafij.inkwell;

import android.Manifest;
import android.app.Activity;
import android.content.ContentResolver;
import android.content.Context;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Environment;
import android.provider.DocumentsContract;
import android.provider.MediaStore;
import android.provider.Settings;

import java.io.File;
import java.util.List;
import java.util.Locale;

/**
 * Finding a note's PDF again. A note keeps both the PDF's content uri and its real file
 * path (like Xournal++). The uri grant is lost when the app is reinstalled; the path
 * keeps working as long as the PDF is not moved and the app has file access.
 */
public final class PdfLinks {
    private PdfLinks() {
    }

    /** Real file path behind a content uri, when it can be worked out; otherwise null. */
    public static String realPath(Context c, Uri uri) {
        if (uri == null) return null;
        try {
            if ("file".equals(uri.getScheme())) return uri.getPath();
            String authority = uri.getAuthority();
            if (DocumentsContract.isDocumentUri(c, uri) || DocumentsContract.isTreeUri(uri)) {
                String id = DocumentsContract.getDocumentId(uri);
                if ("com.android.externalstorage.documents".equals(authority)) {
                    int colon = id.indexOf(':');
                    if (colon < 0) return null;
                    String volume = id.substring(0, colon), rel = id.substring(colon + 1);
                    String root = "primary".equalsIgnoreCase(volume)
                            ? Environment.getExternalStorageDirectory().getPath() : "/storage/" + volume;
                    return rel.isEmpty() ? root : root + "/" + rel;
                }
                if ("com.android.providers.downloads.documents".equals(authority)) {
                    if (id.startsWith("raw:")) return id.substring(4);
                    String num = id.startsWith("msf:") ? id.substring(4) : id;
                    String fromStore = mediaStorePath(c, num);
                    if (fromStore != null) return fromStore;
                    // Most items of the Downloads provider live directly in Download/.
                    String name = Notes.displayName(c, uri, null);
                    return name == null ? null : new File(Environment.getExternalStoragePublicDirectory(
                            Environment.DIRECTORY_DOWNLOADS), name).getPath();
                }
                if ("com.android.providers.media.documents".equals(authority)) {
                    // While the picker's grant lasts, the matching MediaStore item can be read.
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                        try {
                            Uri media = MediaStore.getMediaUri(c, uri);
                            String p = media == null ? null : query(c, media);
                            if (p != null) return p;
                        } catch (RuntimeException ignored) {
                            // fall through
                        }
                    }
                    int colon = id.indexOf(':');
                    return colon < 0 ? null : mediaStorePath(c, id.substring(colon + 1));
                }
                return null;
            }
            if ("media".equals(authority)) return query(c, uri);
        } catch (RuntimeException ignored) {
            // unknown provider
        }
        return null;
    }

    private static String mediaStorePath(Context c, String id) {
        try {
            return query(c, android.content.ContentUris.withAppendedId(
                    MediaStore.Files.getContentUri("external"), Long.parseLong(id)));
        } catch (RuntimeException e) {
            return null;
        }
    }

    @SuppressWarnings("deprecation")
    private static String query(Context c, Uri uri) {
        try (Cursor cur = c.getContentResolver().query(uri, new String[]{MediaStore.MediaColumns.DATA}, null, null, null)) {
            if (cur != null && cur.moveToFirst() && !cur.isNull(0)) return cur.getString(0);
        } catch (RuntimeException ignored) {
            // no access
        }
        return null;
    }

    /** True when the app may open files by path (All files access on Android 11+). */
    public static boolean hasFileAccess(Context c) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) return Environment.isExternalStorageManager();
        return c.checkSelfPermission(Manifest.permission.READ_EXTERNAL_STORAGE) == PackageManager.PERMISSION_GRANTED;
    }

    /** Opens the system page (Android 11+) or the permission prompt for file access. */
    public static void requestFileAccess(Activity a) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            try {
                a.startActivity(new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                        Uri.parse("package:" + a.getPackageName())));
            } catch (RuntimeException e) {
                a.startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
            }
        } else {
            a.requestPermissions(new String[]{Manifest.permission.READ_EXTERNAL_STORAGE}, 7);
        }
    }

    /** The PDF at {@code path}, if it exists and can be read now. */
    public static File readable(String path) {
        if (path == null) return null;
        File f = new File(path);
        return f.isFile() && f.canRead() ? f : null;
    }

    /**
     * Searches shared storage for a file called {@code name} (needs file access). Includes
     * Android/media, where apps such as WhatsApp keep documents; skips Android/data and obb.
     */
    public static File findOnDevice(String name) {
        if (name == null) return null;
        File root = Environment.getExternalStorageDirectory();
        java.util.ArrayDeque<File> queue = new java.util.ArrayDeque<>();
        java.util.ArrayDeque<Integer> depth = new java.util.ArrayDeque<>();
        queue.add(root);
        depth.add(0);
        int visited = 0;
        while (!queue.isEmpty() && visited < 6000) {
            File dir = queue.poll();
            int d = depth.poll();
            File[] kids = dir.listFiles();
            visited++;
            if (kids == null) continue;
            for (File k : kids) {
                if (k.isFile() && k.getName().equalsIgnoreCase(name)) return k;
            }
            if (d >= 8) continue;
            for (File k : kids) {
                if (!k.isDirectory() || k.getName().startsWith(".")) continue;
                String rel = k.getPath().substring(root.getPath().length());
                if (rel.startsWith("/Android/data") || rel.startsWith("/Android/obb")) continue;
                queue.add(k);
                depth.add(d + 1);
            }
        }
        return null;
    }

    /** A PDF called {@code name} in the library tree, searching from {@code startId}. */
    public static SafDocs.Entry findByName(ContentResolver cr, Uri tree, String startId, String name, int depth) {
        if (tree == null || name == null || depth < 0) return null;
        List<SafDocs.Entry> kids = SafDocs.list(cr, tree, startId);
        for (SafDocs.Entry e : kids) {
            if (!e.isDir() && e.name.equalsIgnoreCase(name)) return e;
        }
        for (SafDocs.Entry e : kids) {
            if (e.isDir() && !e.name.startsWith(".") && !"assets".equals(e.name.toLowerCase(Locale.ROOT))) {
                SafDocs.Entry found = findByName(cr, tree, e.docId, name, depth - 1);
                if (found != null) return found;
            }
        }
        return null;
    }
}
