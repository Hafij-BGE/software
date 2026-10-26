package com.hafij.inkwell;

import android.content.ContentResolver;
import android.content.Context;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;

import com.hafij.inkwell.io.InkFile;
import com.hafij.inkwell.model.Doc;
import com.tom_roush.pdfbox.pdmodel.PDDocument;
import com.tom_roush.pdfbox.pdmodel.PDPage;
import com.tom_roush.pdfbox.pdmodel.common.PDRectangle;

import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.text.SimpleDateFormat;
import java.util.Date;
import java.util.Locale;

/** The note library: .ink files and folders under the app's external files directory. */
public final class Notes {
    private Notes() {
    }

    public static File root(Context c) {
        File d = c.getExternalFilesDir("Notes");
        if (d == null) d = new File(c.getFilesDir(), "Notes");
        if (!d.isDirectory()) d.mkdirs();
        return d;
    }

    public static String title(File note) {
        String n = note.getName();
        return n.endsWith(InkFile.EXT) ? n.substring(0, n.length() - InkFile.EXT.length()) : n;
    }

    public static boolean isNote(File f) {
        return f.isFile() && f.getName().endsWith(InkFile.EXT) && !f.getName().startsWith(".");
    }

    /** A file name that does not exist yet in {@code dir}. */
    public static File unique(File dir, String base, String ext) {
        base = sanitize(base);
        File f = new File(dir, base + ext);
        for (int i = 2; f.exists(); i++) f = new File(dir, base + " (" + i + ")" + ext);
        return f;
    }

    public static String sanitize(String s) {
        String t = s.replaceAll("[\\\\/:*?\"<>|\\n\\r\\t]", "_").trim();
        if (t.isEmpty()) t = "Untitled";
        if (t.length() > 120) t = t.substring(0, 120);
        return t;
    }

    public static String defaultName() {
        return "Note " + new SimpleDateFormat("yyyy-MM-dd HH.mm", Locale.getDefault()).format(new Date());
    }

    public static File createBlank(File dir, String name) throws IOException {
        File f = unique(dir, name, InkFile.EXT);
        InkFile.write(Doc.blank(), f, null);
        return f;
    }

    /** Creates a note that annotates a copy of the PDF at {@code uri}. */
    public static File importPdf(Context c, File dir, Uri uri) throws IOException {
        String name = stripExt(displayName(c, uri, "Document.pdf"));
        File note = unique(dir, name, InkFile.EXT);
        File bg = InkFile.bgPdfFor(note);
        copy(c.getContentResolver(), uri, bg);
        try {
            Doc doc = pdfDoc(bg);
            InkFile.write(doc, note, null);
        } catch (IOException | RuntimeException e) {
            bg.delete();
            note.delete();
            throw e instanceof IOException ? (IOException) e : new IOException(e.getMessage(), e);
        }
        return note;
    }

    /** One page per PDF page, sized like the PDF page as displayed. */
    public static Doc pdfDoc(File pdf) throws IOException {
        Doc doc = new Doc();
        doc.pdf = pdf;
        try (PDDocument d = PDDocument.load(pdf)) {
            int i = 0;
            for (PDPage p : d.getPages()) {
                PDRectangle crop = p.getCropBox();
                int rot = ((p.getRotation() % 360) + 360) % 360;
                boolean swap = rot == 90 || rot == 270;
                Doc.Page pg = Doc.Page.blank(swap ? crop.getHeight() : crop.getWidth(),
                        swap ? crop.getWidth() : crop.getHeight());
                pg.bg.type = Doc.Background.Type.PDF;
                pg.bg.pdfPage = i++;
                doc.pages.add(pg);
            }
        }
        if (doc.pages.isEmpty()) throw new IOException("The PDF has no pages");
        return doc;
    }

    /** Copies an .ink or .xopp file into the library. */
    public static File importNote(Context c, File dir, Uri uri) throws IOException {
        String name = stripExt(displayName(c, uri, "Imported.ink"));
        File note = unique(dir, name, InkFile.EXT);
        copy(c.getContentResolver(), uri, note);
        try {
            InkFile.read(note);
        } catch (IOException e) {
            note.delete();
            throw e;
        }
        return note;
    }

    public static String displayName(Context c, Uri uri, String fallback) {
        String name = null;
        try (Cursor cur = c.getContentResolver().query(uri, new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
            if (cur != null && cur.moveToFirst()) name = cur.getString(0);
        } catch (RuntimeException ignored) {
            // fall back below
        }
        if (name == null) name = uri.getLastPathSegment();
        return name == null ? fallback : name;
    }

    public static String stripExt(String n) {
        int dot = n.lastIndexOf('.');
        return dot > 0 ? n.substring(0, dot) : n;
    }

    public static void copy(ContentResolver cr, Uri from, File to) throws IOException {
        try (InputStream in = cr.openInputStream(from); OutputStream out = new FileOutputStream(to)) {
            if (in == null) throw new IOException("Cannot read " + from);
            pipe(in, out);
        }
    }

    public static void copy(File from, File to) throws IOException {
        try (InputStream in = new FileInputStream(from); OutputStream out = new FileOutputStream(to)) {
            pipe(in, out);
        }
    }

    public static void copy(ContentResolver cr, File from, Uri to) throws IOException {
        try (InputStream in = new FileInputStream(from); OutputStream out = cr.openOutputStream(to, "wt")) {
            if (out == null) throw new IOException("Cannot write " + to);
            pipe(in, out);
        }
    }

    private static void pipe(InputStream in, OutputStream out) throws IOException {
        byte[] buf = new byte[1 << 16];
        int n;
        while ((n = in.read(buf)) > 0) out.write(buf, 0, n);
    }

    /** Renames a note together with its attached PDF. Returns the new file. */
    public static File rename(File note, String newName) throws IOException {
        File target = new File(note.getParentFile(), sanitize(newName) + InkFile.EXT);
        if (target.equals(note)) return note;
        if (target.exists()) throw new IOException("A note with that name already exists");
        return moveTo(note, target);
    }

    public static File move(File note, File dir) throws IOException {
        File target = unique(dir, title(note), InkFile.EXT);
        return moveTo(note, target);
    }

    private static File moveTo(File note, File target) throws IOException {
        File bg = InkFile.bgPdfFor(note);
        if (!note.renameTo(target)) throw new IOException("Could not move " + note.getName());
        if (bg.exists() && !bg.renameTo(InkFile.bgPdfFor(target))) {
            target.renameTo(note);
            throw new IOException("Could not move the attached PDF");
        }
        return target;
    }

    public static File duplicate(File note) throws IOException {
        File target = unique(note.getParentFile(), title(note) + " copy", InkFile.EXT);
        copy(note, target);
        File bg = InkFile.bgPdfFor(note);
        if (bg.exists()) copy(bg, InkFile.bgPdfFor(target));
        return target;
    }

    public static void delete(File note) {
        InkFile.bgPdfFor(note).delete();
        note.delete();
    }

    public static boolean deleteFolder(File dir) {
        File[] kids = dir.listFiles();
        if (kids != null) {
            for (File k : kids) {
                if (k.isDirectory()) deleteFolder(k);
                else k.delete();
            }
        }
        return dir.delete();
    }
}
