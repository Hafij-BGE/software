package com.hafij.inkwell.io;

import android.graphics.RectF;
import android.util.Base64;

import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.Element;
import com.hafij.inkwell.model.ImageEl;
import com.hafij.inkwell.model.Stroke;
import com.hafij.inkwell.model.TextEl;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.Locale;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * Sketch exchange with other apps (Markd). The first page is written as an SVG cropped
 * to its contents; the whole note travels gzipped and base64-encoded in
 * &lt;metadata&gt;&lt;inkwell:note&gt;, so the image can be opened again for editing.
 */
public final class SvgSketch {
    public static final String NS = "https://github.com/Hafij-BGE/inkwell";
    private static final Pattern NOTE = Pattern.compile("<inkwell:note[^>]*>([A-Za-z0-9+/=\\s]+)</inkwell:note>");
    private static final float PAD = 12;

    private SvgSketch() {
    }

    /** The gzipped .ink bytes embedded in an Inkwell SVG, or null. */
    public static byte[] embeddedNote(String svg) {
        if (svg == null) return null;
        Matcher m = NOTE.matcher(svg);
        if (!m.find()) return null;
        try {
            return Base64.decode(m.group(1).replaceAll("\\s", ""), Base64.DEFAULT);
        } catch (IllegalArgumentException e) {
            return null;
        }
    }

    /** Bounds of everything on the first page, or null when it is empty. */
    public static RectF contentBounds(Doc doc) {
        if (doc.pages.isEmpty()) return null;
        RectF r = null;
        for (Doc.Layer l : doc.pages.get(0).layers) {
            if (!l.visible) continue;
            for (Element e : l.elements) {
                RectF b = e.bounds();
                if (r == null) r = new RectF(b);
                else r.union(b);
            }
        }
        return r;
    }

    public static String write(Doc doc, boolean paper) throws IOException {
        RectF box = contentBounds(doc);
        if (box == null) throw new IOException("The sketch is empty");
        box.inset(-PAD, -PAD);
        box.set((float) Math.floor(box.left), (float) Math.floor(box.top), (float) Math.ceil(box.right), (float) Math.ceil(box.bottom));

        ByteArrayOutputStream note = new ByteArrayOutputStream();
        InkFile.write(doc, note, null);

        StringBuilder s = new StringBuilder(4096);
        s.append(String.format(Locale.US,
                "<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:inkwell=\"%s\" viewBox=\"%s %s %s %s\" width=\"%s\" height=\"%s\">",
                NS, f(box.left), f(box.top), f(box.width()), f(box.height()), f(box.width()), f(box.height())));
        s.append("<metadata><inkwell:note version=\"1\" encoding=\"gzip-base64\">")
                .append(Base64.encodeToString(note.toByteArray(), Base64.NO_WRAP))
                .append("</inkwell:note></metadata>");
        if (paper) {
            s.append(String.format(Locale.US, "<rect x=\"%s\" y=\"%s\" width=\"%s\" height=\"%s\" fill=\"#ffffff\"/>",
                    f(box.left), f(box.top), f(box.width()), f(box.height())));
        }
        for (Doc.Layer l : doc.pages.get(0).layers) {
            if (!l.visible) continue;
            for (Element e : l.elements) {
                if (e instanceof Stroke) stroke(s, (Stroke) e, paper);
                else if (e instanceof TextEl) text(s, (TextEl) e);
                else if (e instanceof ImageEl) image(s, (ImageEl) e);
            }
        }
        s.append("</svg>");
        return s.toString();
    }

    private static String f(float v) {
        return InkFile.fmt(Math.round(v * 100) / 100f);
    }

    private static String rgb(int argb) {
        return String.format(Locale.US, "#%06x", argb & 0xffffff);
    }

    private static float alpha(int argb) {
        return (argb >>> 24) / 255f;
    }

    private static void stroke(StringBuilder s, Stroke st, boolean paper) {
        int n = st.count();
        if (n == 0) return;
        boolean whiteout = Stroke.ERASER.equals(st.tool);
        if (whiteout && !paper) return;
        int col = whiteout ? 0xffffffff : st.color;
        boolean hl = Stroke.HIGHLIGHTER.equals(st.tool);
        String blend = hl ? " style=\"mix-blend-mode:multiply\"" : "";
        float a = alpha(col);
        float[] p = st.pts;
        float[] dash = st.dashPattern(st.width);
        boolean pressure = st.pressures != null && st.pressures.length == n && dash == null && n > 1;

        if (st.fill >= 0 && n > 2) {
            s.append("<path d=\"").append(polyline(p, n)).append("Z\" fill=\"").append(rgb(col))
                    .append("\" fill-opacity=\"").append(f(a * st.fill / 255f)).append("\" stroke=\"none\"").append(blend).append("/>");
        }
        if (pressure) {
            s.append("<path d=\"").append(outline(p, st.pressures, n)).append("\" fill=\"").append(rgb(col)).append('"');
            if (a < 1) s.append(" fill-opacity=\"").append(f(a)).append('"');
            s.append(blend).append("/>");
            return;
        }
        String cap = "butt".equals(st.cap) ? "butt" : "square".equals(st.cap) ? "square" : "round";
        s.append("<path d=\"").append(polyline(p, n)).append("\" fill=\"none\" stroke=\"").append(rgb(col))
                .append("\" stroke-width=\"").append(f(st.width)).append("\" stroke-linecap=\"").append(cap)
                .append("\" stroke-linejoin=\"round\"");
        if (a < 1) s.append(" stroke-opacity=\"").append(f(a)).append('"');
        if (dash != null) {
            s.append(" stroke-dasharray=\"");
            for (int i = 0; i < dash.length; i++) s.append(i == 0 ? "" : " ").append(f(dash[i]));
            s.append('"');
        }
        s.append(blend).append("/>");
    }

    private static String polyline(float[] p, int n) {
        StringBuilder d = new StringBuilder(n * 14);
        d.append('M').append(f(p[0])).append(' ').append(f(p[1]));
        if (n == 1) d.append('L').append(f(p[0] + 0.01f)).append(' ').append(f(p[1]));
        for (int i = 1; i < n; i++) d.append('L').append(f(p[2 * i])).append(' ').append(f(p[2 * i + 1]));
        return d.toString();
    }

    /** Filled outline of a variable-width stroke with round ends. */
    private static String outline(float[] p, float[] w, int n) {
        float[] lx = new float[n], ly = new float[n], rx = new float[n], ry = new float[n];
        for (int i = 0; i < n; i++) {
            int a = Math.max(0, i - 1), b = Math.min(n - 1, i + 1);
            float dx = p[2 * b] - p[2 * a], dy = p[2 * b + 1] - p[2 * a + 1];
            float len = (float) Math.hypot(dx, dy);
            float nx = len == 0 ? 0 : -dy / len, ny = len == 0 ? 1 : dx / len;
            float h = w[i] / 2;
            lx[i] = p[2 * i] + nx * h;
            ly[i] = p[2 * i + 1] + ny * h;
            rx[i] = p[2 * i] - nx * h;
            ry[i] = p[2 * i + 1] - ny * h;
        }
        StringBuilder d = new StringBuilder(n * 30);
        d.append('M').append(f(lx[0])).append(' ').append(f(ly[0]));
        for (int i = 1; i < n; i++) d.append('L').append(f(lx[i])).append(' ').append(f(ly[i]));
        float re = w[n - 1] / 2;
        d.append('A').append(f(re)).append(' ').append(f(re)).append(" 0 0 0 ").append(f(rx[n - 1])).append(' ').append(f(ry[n - 1]));
        for (int i = n - 2; i >= 0; i--) d.append('L').append(f(rx[i])).append(' ').append(f(ry[i]));
        float rs = w[0] / 2;
        d.append('A').append(f(rs)).append(' ').append(f(rs)).append(" 0 0 0 ").append(f(lx[0])).append(' ').append(f(ly[0]));
        return d.append('Z').toString();
    }

    private static void text(StringBuilder s, TextEl t) {
        if (t.text.isEmpty()) return;
        float[] m = t.metrics();
        String font = t.font == null ? "" : t.font.toLowerCase(Locale.ROOT);
        String family = font.contains("mono") ? "monospace" : font.contains("serif") && !font.contains("sans") ? "serif" : "sans-serif";
        s.append("<text font-family=\"").append(family).append("\" font-size=\"").append(f(t.size))
                .append("\" fill=\"").append(rgb(t.color)).append('"');
        if (font.contains("bold")) s.append(" font-weight=\"bold\"");
        if (font.contains("italic")) s.append(" font-style=\"italic\"");
        if (alpha(t.color) < 1) s.append(" fill-opacity=\"").append(f(alpha(t.color))).append('"');
        s.append(" xml:space=\"preserve\">");
        String[] lines = t.text.split("\n", -1);
        for (int i = 0; i < lines.length; i++) {
            s.append("<tspan x=\"").append(f(t.x)).append("\" y=\"").append(f(t.y + m[0] + i * m[1])).append("\">")
                    .append(esc(lines[i])).append("</tspan>");
        }
        s.append("</text>");
    }

    private static void image(StringBuilder s, ImageEl im) {
        byte[] b = im.bytes();
        if (b.length < 4) return;
        String mime = (b[0] & 0xff) == 0xff && (b[1] & 0xff) == 0xd8 ? "image/jpeg"
                : (b[0] & 0xff) == 0x89 && b[1] == 'P' ? "image/png"
                : b[0] == 'G' && b[1] == 'I' ? "image/gif" : "image/webp";
        RectF r = im.bounds();
        s.append("<image x=\"").append(f(r.left)).append("\" y=\"").append(f(r.top)).append("\" width=\"").append(f(r.width()))
                .append("\" height=\"").append(f(r.height())).append("\" preserveAspectRatio=\"none\" href=\"data:")
                .append(mime).append(";base64,").append(im.data.replaceAll("\\s", "")).append("\"/>");
    }

    private static String esc(String v) {
        return v.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;").replace("\"", "&quot;");
    }
}
