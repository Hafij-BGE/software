package com.hafij.inkwell.model;

import java.io.File;
import java.util.ArrayList;
import java.util.List;

/** An in-memory note: Xournal++ page/layer/element structure. */
public class Doc {
    public static final float A4_W = 595.2756f;
    public static final float A4_H = 841.8898f;

    public final List<Page> pages = new ArrayList<>();
    /** Background PDF on disk, or null when no page uses one (or it is missing). */
    public File pdf;
    /** The PDF filename recorded in the note, used in "PDF not found" messages. */
    public String pdfName;
    /** Content uri of the linked PDF (it stays where it is), or null for an attached copy. */
    public String pdfUri;
    /** Real file path of the linked PDF when known (written as Xournal++'s absolute filename). */
    public String pdfPath;
    /** True if pages reference a PDF that could not be found. */
    public boolean pdfMissing;

    public static Doc blank() {
        Doc d = new Doc();
        d.pages.add(Page.blank(A4_W, A4_H));
        return d;
    }

    /**
     * Copy that is safe to write on another thread: new page/layer lists, shared elements
     * (elements are replaced, never changed, once they are in a layer).
     */
    public Doc snapshot() {
        Doc d = new Doc();
        d.pdf = pdf;
        d.pdfName = pdfName;
        d.pdfUri = pdfUri;
        d.pdfPath = pdfPath;
        d.pdfMissing = pdfMissing;
        for (Page p : pages) {
            Page c = new Page();
            c.width = p.width;
            c.height = p.height;
            c.bg = p.bg;
            c.activeLayer = p.activeLayer;
            for (Layer l : p.layers) {
                Layer nl = new Layer();
                nl.name = l.name;
                nl.visible = l.visible;
                nl.elements = new ArrayList<>(l.elements);
                c.layers.add(nl);
            }
            d.pages.add(c);
        }
        return d;
    }

    public boolean usesPdf() {
        for (Page p : pages) if (p.bg.type == Background.Type.PDF) return true;
        return false;
    }

    public static class Page {
        public float width = A4_W;
        public float height = A4_H;
        public Background bg = new Background();
        public final List<Layer> layers = new ArrayList<>();
        public int activeLayer;

        public static Page blank(float w, float h) {
            Page p = new Page();
            p.width = w;
            p.height = h;
            p.layers.add(new Layer());
            return p;
        }

        public Layer layer() {
            if (layers.isEmpty()) layers.add(new Layer());
            if (activeLayer < 0 || activeLayer >= layers.size()) activeLayer = layers.size() - 1;
            return layers.get(activeLayer);
        }

        public boolean isEmpty() {
            for (Layer l : layers) if (!l.elements.isEmpty()) return false;
            return true;
        }

        /** Copy with the same background and copies of every element. */
        public Page copy() {
            Page p = new Page();
            p.width = width;
            p.height = height;
            p.bg = bg.copy();
            for (Layer l : layers) {
                Layer nl = new Layer();
                nl.name = l.name;
                nl.visible = l.visible;
                for (Element e : l.elements) nl.elements.add(e.copy());
                p.layers.add(nl);
            }
            p.activeLayer = activeLayer;
            return p;
        }

        /** Empty white page with the same size. */
        public Page blankLike() {
            return blank(width, height);
        }
    }

    public static class Layer {
        public String name;
        public boolean visible = true;
        public List<Element> elements = new ArrayList<>();
    }

    public static class Background {
        public enum Type {SOLID, PDF, PIXMAP}

        public Type type = Type.SOLID;
        /** plain, lined, ruled, graph, dotted, staves, isodotted, isograph. */
        public String style = "plain";
        public int color = 0xffffffff;
        /** Template config string such as "m1=40,rm=1", kept verbatim. */
        public String config;
        /** Zero-based PDF page index. */
        public int pdfPage;
        /** Pixmap backgrounds: original attributes, kept for round-tripping. */
        public String pixDomain, pixFilename;

        public Background copy() {
            Background b = new Background();
            b.type = type;
            b.style = style;
            b.color = color;
            b.config = config;
            b.pdfPage = pdfPage;
            b.pixDomain = pixDomain;
            b.pixFilename = pixFilename;
            return b;
        }
    }
}
