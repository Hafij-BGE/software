package com.hafij.inkwell.pdf;

import android.graphics.Bitmap;

import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.Element;
import com.hafij.inkwell.model.ImageEl;
import com.hafij.inkwell.model.Stroke;
import com.hafij.inkwell.model.Template;
import com.hafij.inkwell.model.TextEl;
import com.tom_roush.pdfbox.cos.COSBase;
import com.tom_roush.pdfbox.cos.COSDictionary;
import com.tom_roush.pdfbox.cos.COSName;
import com.tom_roush.pdfbox.pdmodel.PDDocument;
import com.tom_roush.pdfbox.pdmodel.PDPage;
import com.tom_roush.pdfbox.pdmodel.PDPageContentStream;
import com.tom_roush.pdfbox.pdmodel.PDResources;
import com.tom_roush.pdfbox.pdmodel.common.PDRectangle;
import com.tom_roush.pdfbox.pdmodel.font.PDFont;
import com.tom_roush.pdfbox.pdmodel.font.PDType0Font;
import com.tom_roush.pdfbox.pdmodel.font.PDType1Font;
import com.tom_roush.pdfbox.pdmodel.graphics.blend.BlendMode;
import com.tom_roush.pdfbox.pdmodel.graphics.color.PDColor;
import com.tom_roush.pdfbox.pdmodel.graphics.color.PDDeviceRGB;
import com.tom_roush.pdfbox.pdmodel.graphics.image.JPEGFactory;
import com.tom_roush.pdfbox.pdmodel.graphics.image.LosslessFactory;
import com.tom_roush.pdfbox.pdmodel.graphics.image.PDImageXObject;
import com.tom_roush.pdfbox.pdmodel.graphics.state.PDExtendedGraphicsState;
import com.tom_roush.pdfbox.util.Matrix;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * Vector PDF export. PDF background pages are copied as they are (text stays selectable);
 * ink, text and images are drawn into the page content so every viewer shows them.
 */
public final class PdfExport {
    public interface Progress {
        void onPage(int done, int total);
    }

    private PdfExport() {
    }

    public static void export(Doc doc, File out, Progress progress) throws IOException {
        PDDocument src = null;
        try (PDDocument dst = new PDDocument()) {
            if (doc.pdf != null && doc.usesPdf()) {
                src = PDDocument.load(doc.pdf);
                if (src.isEncrypted()) src.setAllSecurityToBeRemoved(true);
            }
            dst.getDocumentInformation().setCreator("Inkwell");
            Ctx ctx = new Ctx(dst);
            int total = doc.pages.size();
            for (int i = 0; i < total; i++) {
                Doc.Page pg = doc.pages.get(i);
                PDPage pd;
                Matrix m;
                boolean pdfBg = pg.bg.type == Doc.Background.Type.PDF && src != null
                        && pg.bg.pdfPage < src.getNumberOfPages();
                if (pdfBg) {
                    PDPage sp = src.getPage(pg.bg.pdfPage);
                    pd = clonePage(sp);
                    m = pdfMatrix(sp, pg);
                } else {
                    pd = new PDPage(new PDRectangle(pg.width, pg.height));
                    pd.setResources(new PDResources());
                    m = new Matrix(1, 0, 0, -1, 0, pg.height);
                }
                dst.addPage(pd);
                try (PDPageContentStream cs = new PDPageContentStream(dst, pd,
                        PDPageContentStream.AppendMode.APPEND, true, true)) {
                    cs.transform(m);
                    if (!pdfBg) drawBackground(cs, pg);
                    for (Doc.Layer l : pg.layers) {
                        if (!l.visible) continue;
                        for (Element e : l.elements) draw(cs, ctx, e);
                    }
                }
                if (progress != null) progress.onPage(i + 1, total);
            }
            File tmp = new File(out.getParentFile(), "." + out.getName() + ".part");
            dst.save(tmp);
            if (!tmp.renameTo(out)) {
                tmp.delete();
                throw new IOException("Could not write " + out.getName());
            }
        } finally {
            if (src != null) src.close();
        }
    }

    private static PDPage clonePage(PDPage src) {
        COSDictionary d = new COSDictionary();
        for (Map.Entry<COSName, COSBase> e : src.getCOSObject().entrySet()) {
            COSName k = e.getKey();
            if (k.equals(COSName.ANNOTS) || k.equals(COSName.PARENT) || k.equals(COSName.STRUCT_PARENTS)
                    || k.equals(COSName.B)) continue;
            d.setItem(k, e.getValue());
        }
        PDPage p = new PDPage(d);
        // Inherited attributes from the source page tree.
        p.setMediaBox(src.getMediaBox());
        p.setCropBox(src.getCropBox());
        p.setRotation(src.getRotation());
        PDResources r = src.getResources();
        p.setResources(r != null ? r : new PDResources());
        return p;
    }

    /** Maps note page coordinates (top-left, displayed orientation) to PDF user space. */
    private static Matrix pdfMatrix(PDPage sp, Doc.Page pg) {
        PDRectangle c = sp.getCropBox();
        int rot = ((sp.getRotation() % 360) + 360) % 360;
        float x0 = c.getLowerLeftX(), y0 = c.getLowerLeftY(), w = c.getWidth(), h = c.getHeight();
        boolean swap = rot == 90 || rot == 270;
        float dw = swap ? h : w, dh = swap ? w : h;
        float sx = pg.width > 0 ? dw / pg.width : 1, sy = pg.height > 0 ? dh / pg.height : 1;
        // ux = a*x + c*y + e, uy = b*x + d*y + f, with (x, y) scaled to the PDF page size.
        switch (rot) {
            case 90:
                return new Matrix(0, sx, sy, 0, x0, y0);
            case 180:
                return new Matrix(-sx, 0, 0, sy, x0 + w, y0);
            case 270:
                return new Matrix(0, -sx, -sy, 0, x0 + w, y0 + h);
            default:
                return new Matrix(sx, 0, 0, -sy, x0, y0 + h);
        }
    }

    private static PDColor rgb(int argb) {
        return new PDColor(new float[]{((argb >> 16) & 255) / 255f, ((argb >> 8) & 255) / 255f, (argb & 255) / 255f},
                PDDeviceRGB.INSTANCE);
    }

    private static void drawBackground(PDPageContentStream cs, Doc.Page pg) throws IOException {
        int bg = pg.bg.type == Doc.Background.Type.SOLID ? pg.bg.color : 0xffffffff;
        if ((bg & 0xffffff) != 0xffffff) {
            cs.setNonStrokingColor(rgb(bg));
            cs.addRect(0, 0, pg.width, pg.height);
            cs.fill();
        }
        if (pg.bg.type != Doc.Background.Type.SOLID) return;
        for (Template.Group g : Template.geometry(pg.bg.style, pg.bg.config, pg.width, pg.height)) {
            cs.setStrokingColor(rgb(g.color));
            cs.setLineWidth(g.width);
            cs.setLineCapStyle(g.dots ? 1 : 0);
            float[] c = g.coords;
            if (g.dots) {
                for (int i = 0; i + 1 < c.length; i += 2) {
                    cs.moveTo(c[i], c[i + 1]);
                    cs.lineTo(c[i] + 0.01f, c[i + 1]);
                }
            } else {
                for (int i = 0; i + 3 < c.length; i += 4) {
                    cs.moveTo(c[i], c[i + 1]);
                    cs.lineTo(c[i + 2], c[i + 3]);
                }
            }
            cs.stroke();
        }
    }

    private static void draw(PDPageContentStream cs, Ctx ctx, Element e) throws IOException {
        if (e instanceof Stroke) drawStroke(cs, ctx, (Stroke) e);
        else if (e instanceof TextEl) drawText(cs, ctx, (TextEl) e);
        else if (e instanceof ImageEl) drawImage(cs, ctx, (ImageEl) e);
    }

    private static void drawStroke(PDPageContentStream cs, Ctx ctx, Stroke s) throws IOException {
        int n = s.count();
        if (n == 0) return;
        float[] p = s.pts;
        boolean whiteout = Stroke.ERASER.equals(s.tool);
        int col = whiteout ? 0xffffffff : s.color;
        float alpha = (col >>> 24) / 255f;
        boolean hl = Stroke.HIGHLIGHTER.equals(s.tool);
        float fillAlpha = s.fill >= 0 ? alpha * s.fill / 255f : alpha;
        cs.saveGraphicsState();
        if (alpha < 1f || fillAlpha < 1f || hl) cs.setGraphicsStateParameters(ctx.gs(alpha, fillAlpha, hl));
        cs.setStrokingColor(rgb(col));
        cs.setNonStrokingColor(rgb(col));
        cs.setLineCapStyle("butt".equals(s.cap) ? 0 : "square".equals(s.cap) ? 2 : 1);
        cs.setLineJoinStyle(1);
        if (s.fill >= 0 && n > 2) {
            path(cs, p, n);
            cs.closePath();
            cs.fill();
        }
        float[] dash = s.dashPattern(s.width);
        if (s.pressures != null && s.pressures.length == n && dash == null && alpha >= 1f && n > 1) {
            for (int i = 1; i < n; i++) {
                cs.setLineWidth((s.pressures[i - 1] + s.pressures[i]) / 2f);
                cs.moveTo(p[2 * i - 2], p[2 * i - 1]);
                cs.lineTo(p[2 * i], p[2 * i + 1]);
                cs.stroke();
            }
        } else {
            float w = s.width;
            if (s.pressures != null && s.pressures.length == n) {
                float sum = 0;
                for (float v : s.pressures) sum += v;
                w = sum / n;
            }
            cs.setLineWidth(w);
            if (dash != null) cs.setLineDashPattern(dash, 0);
            path(cs, p, n);
            cs.stroke();
        }
        cs.restoreGraphicsState();
    }

    private static void path(PDPageContentStream cs, float[] p, int n) throws IOException {
        cs.moveTo(p[0], p[1]);
        if (n == 1) cs.lineTo(p[0] + 0.01f, p[1]);
        for (int i = 1; i < n; i++) cs.lineTo(p[2 * i], p[2 * i + 1]);
    }

    private static void drawText(PDPageContentStream cs, Ctx ctx, TextEl t) throws IOException {
        if (t.text.isEmpty()) return;
        float[] m = t.metrics();
        cs.saveGraphicsState();
        float alpha = (t.color >>> 24) / 255f;
        if (alpha < 1f) cs.setGraphicsStateParameters(ctx.gs(1f, alpha, false));
        cs.setNonStrokingColor(rgb(t.color));
        PDFont font = ctx.font();
        float baseline = t.y + m[0];
        for (String line : t.text.split("\n", -1)) {
            if (!line.isEmpty()) {
                cs.beginText();
                cs.setFont(font, t.size);
                // The page matrix flips y; flip glyphs back upright.
                cs.setTextMatrix(new Matrix(1, 0, 0, -1, t.x, baseline));
                cs.showText(ctx.encodable(line));
                cs.endText();
            }
            baseline += m[1];
        }
        cs.restoreGraphicsState();
    }

    private static void drawImage(PDPageContentStream cs, Ctx ctx, ImageEl im) throws IOException {
        PDImageXObject x = ctx.image(im);
        if (x == null) return;
        android.graphics.RectF b = im.bounds();
        cs.saveGraphicsState();
        cs.drawImage(x, new Matrix(b.width(), 0, 0, -b.height(), b.left, b.bottom));
        cs.restoreGraphicsState();
    }

    /** Per-export caches: graphics states, the embedded font and images. */
    private static final class Ctx {
        final PDDocument doc;
        final Map<String, PDExtendedGraphicsState> states = new HashMap<>();
        final Map<String, PDImageXObject> images = new HashMap<>();
        PDFont font;

        Ctx(PDDocument doc) {
            this.doc = doc;
        }

        PDExtendedGraphicsState gs(float stroke, float fill, boolean multiply) {
            String key = stroke + "/" + fill + "/" + multiply;
            PDExtendedGraphicsState g = states.get(key);
            if (g == null) {
                g = new PDExtendedGraphicsState();
                g.setStrokingAlphaConstant(stroke);
                g.setNonStrokingAlphaConstant(fill);
                if (multiply) g.setBlendMode(BlendMode.MULTIPLY);
                states.put(key, g);
            }
            return g;
        }

        PDImageXObject image(ImageEl im) throws IOException {
            PDImageXObject x = images.get(im.data);
            if (x != null) return x;
            byte[] bytes = im.bytes();
            boolean jpeg = bytes.length > 2 && (bytes[0] & 0xff) == 0xff && (bytes[1] & 0xff) == 0xd8;
            if (jpeg) {
                x = JPEGFactory.createFromStream(doc, new ByteArrayInputStream(bytes));
            } else {
                Bitmap b = im.bitmap();
                if (b == null) return null;
                x = LosslessFactory.createFromImage(doc, b);
            }
            images.put(im.data, x);
            return x;
        }

        PDFont font() {
            if (font == null) {
                String[] candidates = {"/system/fonts/Roboto-Regular.ttf", "/system/fonts/RobotoStatic-Regular.ttf",
                        "/system/fonts/NotoSans-Regular.ttf", "/system/fonts/DroidSans.ttf"};
                for (String c : candidates) {
                    File f = new File(c);
                    if (!f.canRead()) continue;
                    try {
                        font = PDType0Font.load(doc, f);
                        break;
                    } catch (IOException | RuntimeException ignored) {
                        // try the next one
                    }
                }
                if (font == null) font = PDType1Font.HELVETICA;
            }
            return font;
        }

        /** Replaces characters the font cannot encode with '?'. */
        String encodable(String s) {
            PDFont f = font();
            try {
                f.encode(s);
                return s;
            } catch (IOException | IllegalArgumentException e) {
                StringBuilder b = new StringBuilder();
                int i = 0;
                while (i < s.length()) {
                    int cp = s.codePointAt(i);
                    String ch = new String(Character.toChars(cp));
                    try {
                        f.encode(ch);
                        b.append(ch);
                    } catch (IOException | IllegalArgumentException e2) {
                        b.append('?');
                    }
                    i += Character.charCount(cp);
                }
                return b.toString();
            }
        }
    }
}
