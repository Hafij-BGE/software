package com.hafij.inkwell.io;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.util.Base64;
import android.util.Xml;

import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.Element;
import com.hafij.inkwell.model.ImageEl;
import com.hafij.inkwell.model.Stroke;
import com.hafij.inkwell.model.TextEl;

import org.xmlpull.v1.XmlPullParser;
import org.xmlpull.v1.XmlPullParserException;

import java.io.BufferedInputStream;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.OutputStreamWriter;
import java.io.Writer;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
import java.util.zip.GZIPInputStream;
import java.util.zip.GZIPOutputStream;
import java.util.zip.ZipEntry;
import java.util.zip.ZipInputStream;

/**
 * Reads and writes notes in the Xournal++ file structure (gzip XML, fileversion 4).
 * The app saves them as .ink; .xopp files from the desktop open the same way.
 * An attached background PDF lives next to the note as "&lt;note&gt;.bg.pdf".
 */
public final class InkFile {
    public static final String EXT = ".ink";
    public static final String BG_SUFFIX = ".bg.pdf";
    /** Background attribute holding the content uri of a linked PDF. */
    public static final String LINK_ATTR = "inkwell-uri";

    private InkFile() {
    }

    public static File bgPdfFor(File note) {
        return new File(note.getPath() + BG_SUFFIX);
    }

    // ---------------------------------------------------------------- reading

    private static InputStream open(File f) throws IOException {
        return open(new FileInputStream(f));
    }

    /** Unwraps gzip or zip-based note data; plain XML passes through. */
    private static InputStream open(InputStream raw) throws IOException {
        BufferedInputStream in = new BufferedInputStream(raw);
        in.mark(4);
        int b0 = in.read(), b1 = in.read();
        in.reset();
        if (b0 == 0x1f && b1 == 0x8b) return new GZIPInputStream(in, 65536);
        if (b0 == 'P' && b1 == 'K') {
            // Zip-based Xournal++ files keep the XML in content.xml.
            ZipInputStream z = new ZipInputStream(in);
            ZipEntry e;
            while ((e = z.getNextEntry()) != null) {
                if ("content.xml".equals(e.getName())) return z;
            }
            z.close();
            throw new IOException("content.xml missing");
        }
        return in;
    }

    public static Doc read(File f) throws IOException {
        try (InputStream in = open(f)) {
            XmlPullParser p = Xml.newPullParser();
            p.setInput(in, null);
            return parse(p, f);
        } catch (XmlPullParserException e) {
            throw new IOException("Not a valid note file: " + e.getMessage(), e);
        }
    }

    private static Doc parse(XmlPullParser p, File noteFile) throws IOException, XmlPullParserException {
        Doc doc = new Doc();
        Doc.Page page = null;
        Doc.Layer layer = null;
        int ev;
        while ((ev = p.next()) != XmlPullParser.END_DOCUMENT) {
            if (ev != XmlPullParser.START_TAG) continue;
            switch (p.getName()) {
                case "page":
                    page = new Doc.Page();
                    page.width = num(p.getAttributeValue(null, "width"), Doc.A4_W);
                    page.height = num(p.getAttributeValue(null, "height"), Doc.A4_H);
                    doc.pages.add(page);
                    layer = null;
                    break;
                case "background":
                    if (page != null) readBackground(p, page, doc, noteFile);
                    break;
                case "layer":
                    if (page == null) break;
                    layer = new Doc.Layer();
                    layer.name = p.getAttributeValue(null, "name");
                    layer.visible = !"false".equals(p.getAttributeValue(null, "visible"));
                    page.layers.add(layer);
                    break;
                case "stroke":
                    if (layer != null) add(layer, readStroke(p));
                    break;
                case "text":
                    if (layer != null) add(layer, readText(p));
                    break;
                case "image":
                case "teximage":
                    if (layer != null) add(layer, readImage(p));
                    break;
                default:
                    break;
            }
        }
        for (Doc.Page pg : doc.pages) {
            if (pg.layers.isEmpty()) pg.layers.add(new Doc.Layer());
            pg.activeLayer = pg.layers.size() - 1;
        }
        if (doc.pages.isEmpty()) doc.pages.add(Doc.Page.blank(Doc.A4_W, Doc.A4_H));
        if (doc.usesPdf() && doc.pdf == null) {
            File attached = bgPdfFor(noteFile);
            if (attached.isFile()) doc.pdf = attached;
            else doc.pdfMissing = true;
        }
        return doc;
    }

    private static void add(Doc.Layer l, Element e) {
        if (e != null) l.elements.add(e);
    }

    private static void readBackground(XmlPullParser p, Doc.Page page, Doc doc, File noteFile) {
        String type = p.getAttributeValue(null, "type");
        Doc.Background bg = page.bg;
        if ("pdf".equals(type)) {
            bg.type = Doc.Background.Type.PDF;
            String no = p.getAttributeValue(null, "pageno");
            bg.pdfPage = Math.max(0, (int) num(no == null ? "1" : no.replaceAll("[^0-9]", ""), 1) - 1);
            String domain = p.getAttributeValue(null, "domain");
            String name = p.getAttributeValue(null, "filename");
            if (domain != null && name != null && doc.pdf == null) {
                doc.pdfName = name;
                String link = p.getAttributeValue(null, LINK_ATTR);
                if (link != null) doc.pdfUri = link;
                if (name.startsWith("/")) doc.pdfPath = name;
                File f = null;
                if ("attach".equals(domain)) {
                    f = new File(noteFile.getPath() + "." + name);
                } else {
                    File abs = new File(name);
                    if (abs.isFile()) f = abs;
                    else f = new File(noteFile.getParentFile(), abs.getName());
                }
                if (!f.isFile()) f = bgPdfFor(noteFile);
                if (f.isFile()) doc.pdf = f;
                else doc.pdfMissing = true;
            }
        } else if ("pixmap".equals(type)) {
            bg.type = Doc.Background.Type.PIXMAP;
            bg.pixDomain = p.getAttributeValue(null, "domain");
            bg.pixFilename = p.getAttributeValue(null, "filename");
        } else {
            bg.type = Doc.Background.Type.SOLID;
            String style = p.getAttributeValue(null, "style");
            bg.style = style == null ? "plain" : style;
            bg.color = bgColor(p.getAttributeValue(null, "color"));
            bg.config = p.getAttributeValue(null, "config");
        }
    }

    private static Stroke readStroke(XmlPullParser p) throws IOException, XmlPullParserException {
        Stroke s = new Stroke();
        String tool = p.getAttributeValue(null, "tool");
        s.tool = tool == null ? Stroke.PEN : tool;
        s.color = color(p.getAttributeValue(null, "color"), 0xff000000);
        String[] w = split(p.getAttributeValue(null, "width"));
        s.width = w.length > 0 ? num(w[0], 1.41f) : 1.41f;
        String fill = p.getAttributeValue(null, "fill");
        s.fill = fill == null ? -1 : (int) num(fill, -1);
        String cap = p.getAttributeValue(null, "capStyle");
        s.cap = cap == null ? "round" : cap;
        s.style = p.getAttributeValue(null, "style");
        String[] pr = split(p.getAttributeValue(null, "pressures"));
        String[] coords = split(p.nextText());
        int n = coords.length / 2;
        float[] pts = new float[n * 2];
        for (int i = 0; i < n * 2; i++) pts[i] = num(coords[i], 0);
        float[] widths = null;
        if (pr.length == n && n > 0) {
            widths = new float[n];
            for (int i = 0; i < n; i++) widths[i] = num(pr[i], s.width);
        } else if (w.length > 1 && n > 1) {
            // Legacy Xournal: one width per segment after the nominal width.
            widths = new float[n];
            for (int i = 0; i < n; i++) widths[i] = num(w[Math.min(Math.max(i, 1), w.length - 1)], s.width);
        }
        s.setPoints(pts, widths);
        return n == 0 ? null : s;
    }

    private static TextEl readText(XmlPullParser p) throws IOException, XmlPullParserException {
        TextEl t = new TextEl();
        String font = p.getAttributeValue(null, "font");
        if (font != null) t.font = font;
        t.size = num(p.getAttributeValue(null, "size"), 12);
        t.x = num(p.getAttributeValue(null, "x"), 0);
        t.y = num(p.getAttributeValue(null, "y"), 0);
        t.color = color(p.getAttributeValue(null, "color"), 0xff000000);
        t.text = p.nextText();
        return t;
    }

    private static ImageEl readImage(XmlPullParser p) throws IOException, XmlPullParserException {
        ImageEl i = new ImageEl();
        boolean tex = "teximage".equals(p.getName());
        i.left = num(p.getAttributeValue(null, "left"), 0);
        i.top = num(p.getAttributeValue(null, "top"), 0);
        i.right = num(p.getAttributeValue(null, "right"), 0);
        i.bottom = num(p.getAttributeValue(null, "bottom"), 0);
        if (tex) {
            String t = p.getAttributeValue(null, "text");
            i.tex = t == null ? "" : t;
        }
        i.data = p.nextText().trim();
        return i;
    }

    /** Reads only the &lt;preview&gt; thumbnail, stopping before the pages. */
    public static Bitmap readPreview(File f) {
        try {
            return readPreview(new FileInputStream(f));
        } catch (IOException e) {
            return null;
        }
    }

    /**
     * The PDF a note annotates: {uri (may be null), file name, absolute path (may be null)}, read from the first PDF
     * background; null for drawings. Closes the stream.
     */
    public static String[] readPdfLink(InputStream raw) {
        try (InputStream in = open(raw)) {
            XmlPullParser p = Xml.newPullParser();
            p.setInput(in, null);
            int ev;
            while ((ev = p.next()) != XmlPullParser.END_DOCUMENT) {
                if (ev != XmlPullParser.START_TAG) continue;
                if ("background".equals(p.getName())) {
                    if (!"pdf".equals(p.getAttributeValue(null, "type"))) return null;
                    String name = p.getAttributeValue(null, "filename");
                    if (name == null) continue;
                    return new String[]{p.getAttributeValue(null, LINK_ATTR), new File(name).getName(),
                            name.startsWith("/") ? name : null};
                }
                if ("layer".equals(p.getName())) continue;
            }
        } catch (IOException | XmlPullParserException | RuntimeException e) {
            return null;
        }
        return null;
    }

    /** Reads only the &lt;preview&gt; thumbnail from a note stream, then closes it. */
    public static Bitmap readPreview(InputStream raw) {
        try (InputStream in = open(raw)) {
            XmlPullParser p = Xml.newPullParser();
            p.setInput(in, null);
            int ev;
            while ((ev = p.next()) != XmlPullParser.END_DOCUMENT) {
                if (ev != XmlPullParser.START_TAG) continue;
                if ("preview".equals(p.getName())) {
                    byte[] b = Base64.decode(p.nextText().trim(), Base64.DEFAULT);
                    return BitmapFactory.decodeByteArray(b, 0, b.length);
                }
                if ("page".equals(p.getName())) return null;
            }
        } catch (IOException | XmlPullParserException | IllegalArgumentException e) {
            return null;
        }
        return null;
    }

    private static String[] split(String s) {
        if (s == null) return new String[0];
        s = s.trim();
        return s.isEmpty() ? new String[0] : s.split("\\s+");
    }

    private static float num(String s, float def) {
        if (s == null) return def;
        try {
            return Float.parseFloat(s.trim());
        } catch (NumberFormatException e) {
            return def;
        }
    }

    private static int color(String s, int def) {
        if (s == null) return def;
        if (s.startsWith("#")) {
            try {
                long v = Long.parseLong(s.substring(1), 16);
                if (s.length() == 9) return (int) (((v & 0xff) << 24) | (v >> 8));
                if (s.length() == 7) return (int) (0xff000000L | v);
            } catch (NumberFormatException e) {
                return def;
            }
        }
        switch (s) {
            case "black": return 0xff000000;
            case "blue": return 0xff3333cc;
            case "red": return 0xffff0000;
            case "green": return 0xff008000;
            case "gray": return 0xff808080;
            case "lightblue": return 0xff00c0ff;
            case "lightgreen": return 0xff00ff00;
            case "magenta": return 0xffff00ff;
            case "orange": return 0xffff8000;
            case "yellow": return 0xffffff00;
            case "white": return 0xffffffff;
            default: return def;
        }
    }

    private static int bgColor(String s) {
        if (s == null) return 0xffffffff;
        switch (s) {
            case "white": return 0xffffffff;
            case "yellow": return 0xffffffa0;
            case "pink": return 0xffffc0d4;
            case "orange": return 0xffffd080;
            case "blue": return 0xffa0e8ff;
            case "green": return 0xff80ffc0;
            default: return color(s, 0xffffffff);
        }
    }

    // ---------------------------------------------------------------- writing

    /** Saves atomically. {@code previewPng} may be null. */
    public static void write(Doc doc, File f, byte[] previewPng) throws IOException {
        File tmp = new File(f.getParentFile(), "." + f.getName() + ".saving");
        try (OutputStream out = new FileOutputStream(tmp)) {
            write(doc, out, previewPng);
        } catch (IOException e) {
            tmp.delete();
            throw e;
        }
        if (!tmp.renameTo(f)) {
            tmp.delete();
            throw new IOException("Could not replace " + f.getName());
        }
    }

    /** Writes the gzipped note to {@code stream} and closes it. */
    public static void write(Doc doc, OutputStream stream, byte[] previewPng) throws IOException {
        try (Writer w = new BufferedWriter(new OutputStreamWriter(
                new GZIPOutputStream(stream, 65536), StandardCharsets.UTF_8), 65536)) {
            w.write("<?xml version=\"1.0\" standalone=\"no\"?>\n");
            w.write("<xournal creator=\"Inkwell 1.0\" fileversion=\"4\">\n");
            w.write("<title>Inkwell note (Xournal++ format)</title>\n");
            if (previewPng != null) {
                w.write("<preview>");
                w.write(Base64.encodeToString(previewPng, Base64.NO_WRAP));
                w.write("</preview>\n");
            }
            boolean pdfWritten = false;
            for (Doc.Page pg : doc.pages) {
                w.write("<page width=\"" + fmt(pg.width) + "\" height=\"" + fmt(pg.height) + "\">\n");
                Doc.Background bg = pg.bg;
                if (bg.type == Doc.Background.Type.PDF) {
                    w.write("<background type=\"pdf\"");
                    if (!pdfWritten) {
                        if (doc.pdfUri != null) {
                            // Linked PDF: the name keeps desktop Xournal++ working when the PDF sits
                            // next to the note; the uri is how the app finds it.
                            String name = doc.pdfPath != null ? doc.pdfPath
                                    : doc.pdfName == null ? "document.pdf" : new File(doc.pdfName).getName();
                            w.write(" domain=\"absolute\" filename=\"" + esc(name) + "\" " + LINK_ATTR + "=\"" + esc(doc.pdfUri) + "\"");
                        } else {
                            w.write(" domain=\"attach\" filename=\"bg.pdf\"");
                        }
                        pdfWritten = true;
                    }
                    w.write(" pageno=\"" + (bg.pdfPage + 1) + "\"/>\n");
                } else if (bg.type == Doc.Background.Type.PIXMAP && bg.pixFilename != null) {
                    w.write("<background type=\"pixmap\" domain=\"" + esc(bg.pixDomain == null ? "absolute" : bg.pixDomain)
                            + "\" filename=\"" + esc(bg.pixFilename) + "\"/>\n");
                } else {
                    w.write("<background type=\"solid\" color=\"" + hex(bg.color) + "\" style=\""
                            + esc(bg.type == Doc.Background.Type.SOLID ? bg.style : "plain") + "\"");
                    if (bg.config != null) w.write(" config=\"" + esc(bg.config) + "\"");
                    w.write("/>\n");
                }
                for (Doc.Layer l : pg.layers) {
                    w.write("<layer");
                    if (l.name != null) w.write(" name=\"" + esc(l.name) + "\"");
                    if (!l.visible) w.write(" visible=\"false\"");
                    w.write(">\n");
                    for (Element e : l.elements) writeElement(w, e);
                    w.write("</layer>\n");
                }
                w.write("</page>\n");
            }
            w.write("</xournal>\n");
        }
    }

    private static void writeElement(Writer w, Element e) throws IOException {
        if (e instanceof Stroke) {
            Stroke s = (Stroke) e;
            int n = s.count();
            if (n == 0) return;
            w.write("<stroke tool=\"" + esc(s.tool) + "\" color=\"" + hex(s.color) + "\" width=\"" + fmt(s.width) + "\"");
            if (s.pressures != null && s.pressures.length == n) {
                StringBuilder b = new StringBuilder();
                for (int i = 0; i < n; i++) {
                    if (i > 0) b.append(' ');
                    b.append(fmt(s.pressures[i]));
                }
                w.write(" pressures=\"" + b + "\"");
            }
            if (s.fill >= 0) w.write(" fill=\"" + s.fill + "\"");
            w.write(" capStyle=\"" + esc(s.cap) + "\"");
            if (s.style != null) w.write(" style=\"" + esc(s.style) + "\"");
            w.write(">");
            StringBuilder b = new StringBuilder(n * 16);
            for (int i = 0; i < n * 2; i++) {
                if (i > 0) b.append(' ');
                b.append(fmt(s.pts[i]));
            }
            w.write(b.toString());
            w.write("</stroke>\n");
        } else if (e instanceof TextEl) {
            TextEl t = (TextEl) e;
            w.write("<text font=\"" + esc(t.font) + "\" size=\"" + fmt(t.size) + "\" x=\"" + fmt(t.x) + "\" y=\""
                    + fmt(t.y) + "\" color=\"" + hex(t.color) + "\">" + esc(t.text) + "</text>\n");
        } else if (e instanceof ImageEl) {
            ImageEl i = (ImageEl) e;
            String tag = i.tex != null ? "teximage" : "image";
            w.write("<" + tag + " left=\"" + fmt(i.left) + "\" top=\"" + fmt(i.top) + "\" right=\"" + fmt(i.right)
                    + "\" bottom=\"" + fmt(i.bottom) + "\"");
            if (i.tex != null) w.write(" text=\"" + esc(i.tex) + "\"");
            w.write(">" + i.data + "</" + tag + ">\n");
        }
    }

    private static String hex(int argb) {
        return String.format(Locale.US, "#%06x%02x", argb & 0xffffff, argb >>> 24);
    }

    static String fmt(float v) {
        if (v == (long) v) return Long.toString((long) v);
        String s = String.format(Locale.US, "%.4f", v);
        int end = s.length();
        while (s.charAt(end - 1) == '0') end--;
        if (s.charAt(end - 1) == '.') end--;
        return s.substring(0, end);
    }

    private static String esc(String s) {
        if (s == null) return "";
        StringBuilder b = new StringBuilder(s.length());
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '<': b.append("&lt;"); break;
                case '>': b.append("&gt;"); break;
                case '&': b.append("&amp;"); break;
                case '"': b.append("&quot;"); break;
                default: b.append(c);
            }
        }
        return b.toString();
    }
}
