package com.hafij.inkwell.editor;

import android.content.SharedPreferences;

/** Current tool and per-tool options, persisted in preferences. */
public class Tools {
    public enum Tool {PEN, HIGHLIGHTER, ERASER, TEXT, SHAPE, SELECT_RECT, SELECT_LASSO, HAND}

    /** Xournal++ default widths (pt): very fine, fine, medium, thick, very thick. */
    public static final float[] PEN_SIZES = {0.42f, 0.85f, 1.41f, 2.26f, 5.67f};
    public static final float[] HL_SIZES = {2.83f, 5.67f, 8.5f, 14.17f, 19.84f};
    public static final float[] ERASER_SIZES = {3f, 6f, 10f, 16f, 24f};
    public static final String[] STYLES = {null, "dash", "dashdot", "dot"};

    /** Xournal++ palette (palettes/xournalpp.gpl). */
    public static final int[] PALETTE = {
            0xff000000, 0xff002e99, 0xffed5353, 0xff3a9104, 0xff64baff, 0xff9bdb4d,
            0xffcd9ef7, 0xffffa154, 0xffffe16b, 0xff808080, 0xffffffff,
    };

    /** Highlighter strokes are stored with alpha 0x7f, as Xournal++ does. */
    public static final int HL_ALPHA = 0x7f000000;

    public Tool tool = Tool.PEN;
    public int penColor = 0xff000000;
    public int hlColor = 0xffffe16b;
    public int textColor = 0xff000000;
    public int penSize = 2, hlSize = 2, eraserSize = 2;
    public int penStyle;
    public boolean fill;
    /** Opacity 0..255: pen and shapes, highlighter (Xournal++ uses 0x7f), and fills. */
    public int penAlpha = 255, hlAlpha = 0x7f, fillAlpha = 128;
    public int shape = Shapes.LINE;
    /** true = erase whole strokes, false = erase only the touched part. */
    public boolean eraseWhole;
    public boolean recognizer;
    public boolean pressure = true;
    public boolean stylusOnly;
    public float textSize = 14;

    public float penWidth() {
        return PEN_SIZES[penSize];
    }

    public float hlWidth() {
        return HL_SIZES[hlSize];
    }

    public float eraserWidth() {
        return ERASER_SIZES[eraserSize];
    }

    /** Colour used by the current tool; highlighter is half transparent like Xournal++. */
    public int currentColor() {
        switch (tool) {
            case HIGHLIGHTER:
                return hlColor;
            case TEXT:
                return textColor;
            default:
                return penColor;
        }
    }

    public void setCurrentColor(int c) {
        switch (tool) {
            case HIGHLIGHTER:
                hlColor = c;
                break;
            case TEXT:
                textColor = c;
                break;
            default:
                penColor = c;
                textColor = c;
        }
    }

    public int sizeIndex() {
        switch (tool) {
            case HIGHLIGHTER:
                return hlSize;
            case ERASER:
                return eraserSize;
            default:
                return penSize;
        }
    }

    public void setSizeIndex(int i) {
        switch (tool) {
            case HIGHLIGHTER:
                hlSize = i;
                break;
            case ERASER:
                eraserSize = i;
                break;
            default:
                penSize = i;
        }
    }

    public void load(SharedPreferences p) {
        try {
            tool = Tool.valueOf(p.getString("tool", tool.name()));
        } catch (IllegalArgumentException ignored) {
            tool = Tool.PEN;
        }
        if (tool == Tool.HAND || tool == Tool.TEXT) tool = Tool.PEN;
        penColor = p.getInt("penColor", penColor);
        hlColor = p.getInt("hlColor", hlColor);
        textColor = p.getInt("textColor", textColor);
        penSize = clamp(p.getInt("penSize", penSize));
        hlSize = clamp(p.getInt("hlSize", hlSize));
        eraserSize = clamp(p.getInt("eraserSize", eraserSize));
        penStyle = Math.max(0, Math.min(STYLES.length - 1, p.getInt("penStyle", penStyle)));
        fill = p.getBoolean("fill", fill);
        penAlpha = clampAlpha(p.getInt("penAlpha", penAlpha));
        hlAlpha = clampAlpha(p.getInt("hlAlpha", hlAlpha));
        fillAlpha = clampAlpha(p.getInt("fillAlpha", fillAlpha));
        shape = p.getInt("shape", shape);
        eraseWhole = p.getBoolean("eraseWhole", eraseWhole);
        recognizer = p.getBoolean("recognizer", recognizer);
        pressure = p.getBoolean("pressure", pressure);
        stylusOnly = p.getBoolean("stylusOnly", stylusOnly);
        textSize = p.getFloat("textSize", textSize);
    }

    public void save(SharedPreferences p) {
        p.edit()
                .putString("tool", tool.name())
                .putInt("penColor", penColor)
                .putInt("hlColor", hlColor)
                .putInt("textColor", textColor)
                .putInt("penSize", penSize)
                .putInt("hlSize", hlSize)
                .putInt("eraserSize", eraserSize)
                .putInt("penStyle", penStyle)
                .putBoolean("fill", fill)
                .putInt("penAlpha", penAlpha)
                .putInt("hlAlpha", hlAlpha)
                .putInt("fillAlpha", fillAlpha)
                .putInt("shape", shape)
                .putBoolean("eraseWhole", eraseWhole)
                .putBoolean("recognizer", recognizer)
                .putBoolean("pressure", pressure)
                .putBoolean("stylusOnly", stylusOnly)
                .putFloat("textSize", textSize)
                .apply();
    }

    private static int clampAlpha(int a) {
        return Math.max(13, Math.min(255, a));
    }

    private static int clamp(int i) {
        return Math.max(0, Math.min(4, i));
    }
}
