package com.hafij.inkwell;

import android.app.Activity;
import android.content.ClipData;
import android.content.ContentProvider;
import android.content.ContentValues;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import android.webkit.MimeTypeMap;
import android.widget.Toast;

import java.io.File;
import java.io.FileNotFoundException;
import java.io.IOException;

/** Read-only provider for sharing exported PDFs and note files. */
public class ShareProvider extends ContentProvider {
    /** Matches the manifest: "${applicationId}.inkwell.share". */
    public static String authority(Context c) {
        return c.getPackageName() + ".inkwell.share";
    }

    private static File rootFor(Context c, String key) {
        if ("c".equals(key)) return new File(c.getCacheDir(), "share");
        if ("n".equals(key)) return Notes.root(c);
        return null;
    }

    public static Uri uriFor(Context c, File f) throws IOException {
        String path = f.getCanonicalPath();
        for (String key : new String[]{"c", "n"}) {
            String root = rootFor(c, key).getCanonicalPath();
            if (path.startsWith(root + File.separator)) {
                return new Uri.Builder().scheme("content").authority(authority(c)).appendPath(key)
                        .appendEncodedPath(Uri.encode(path.substring(root.length() + 1), "/")).build();
            }
        }
        throw new IOException("Cannot share " + f.getName());
    }

    public static void share(Activity a, File f, String mime) {
        try {
            Uri uri = uriFor(a, f);
            Intent i = new Intent(Intent.ACTION_SEND);
            i.setType(mime);
            i.putExtra(Intent.EXTRA_STREAM, uri);
            i.setClipData(ClipData.newRawUri(f.getName(), uri));
            i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
            a.startActivity(Intent.createChooser(i, "Share " + f.getName()));
        } catch (IOException | RuntimeException e) {
            Toast.makeText(a, "Cannot share: " + e.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    private File file(Uri uri) throws FileNotFoundException {
        if (!authority(getContext()).equals(uri.getAuthority()) || uri.getPathSegments().size() < 2) throw new FileNotFoundException();
        File root = rootFor(getContext(), uri.getPathSegments().get(0));
        if (root == null) throw new FileNotFoundException();
        String rel = uri.getEncodedPath().substring(3); // after "/x/"
        File f = new File(root, Uri.decode(rel));
        try {
            if (!f.getCanonicalPath().startsWith(root.getCanonicalPath() + File.separator)) throw new FileNotFoundException();
        } catch (IOException e) {
            throw new FileNotFoundException();
        }
        return f;
    }

    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public ParcelFileDescriptor openFile(Uri uri, String mode) throws FileNotFoundException {
        if (!"r".equals(mode)) throw new FileNotFoundException("read only");
        return ParcelFileDescriptor.open(file(uri), ParcelFileDescriptor.MODE_READ_ONLY);
    }

    @Override
    public Cursor query(Uri uri, String[] projection, String sel, String[] args, String sort) {
        File f;
        try {
            f = file(uri);
        } catch (FileNotFoundException e) {
            return null;
        }
        String[] cols = projection != null ? projection : new String[]{OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE};
        MatrixCursor c = new MatrixCursor(cols, 1);
        Object[] row = new Object[cols.length];
        for (int i = 0; i < cols.length; i++) {
            if (OpenableColumns.DISPLAY_NAME.equals(cols[i])) row[i] = f.getName();
            else if (OpenableColumns.SIZE.equals(cols[i])) row[i] = f.length();
        }
        c.addRow(row);
        return c;
    }

    @Override
    public String getType(Uri uri) {
        String name = uri.getLastPathSegment();
        if (name != null && name.endsWith(".ink")) return "application/octet-stream";
        String ext = name == null ? null : MimeTypeMap.getFileExtensionFromUrl(name);
        String t = ext == null ? null : MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext.toLowerCase());
        return t != null ? t : "application/octet-stream";
    }

    @Override
    public Uri insert(Uri uri, ContentValues v) {
        throw new UnsupportedOperationException();
    }

    @Override
    public int delete(Uri uri, String s, String[] a) {
        throw new UnsupportedOperationException();
    }

    @Override
    public int update(Uri uri, ContentValues v, String s, String[] a) {
        throw new UnsupportedOperationException();
    }
}
