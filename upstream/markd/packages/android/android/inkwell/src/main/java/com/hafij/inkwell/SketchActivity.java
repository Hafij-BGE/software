package com.hafij.inkwell;

/**
 * Drawing surface for other apps (Markd). Start it with action
 * {@code com.hafij.inkwell.action.SKETCH} for a result; it returns a content URI to an SVG
 * that embeds the note, so passing that note back opens the sketch for editing.
 *
 * <p>Extras: {@link #EXTRA_INK} (base64 gzipped .ink) or {@link #EXTRA_SVG} (an earlier
 * result) to edit, and {@link #EXTRA_PAPER} (default true) for a white background.
 */
public class SketchActivity extends EditorActivity {
    public static final String ACTION = "com.hafij.inkwell.action.SKETCH";
    public static final String EXTRA_INK = "com.hafij.inkwell.extra.INK";
    public static final String EXTRA_SVG = "com.hafij.inkwell.extra.SVG";
    public static final String EXTRA_PAPER = "com.hafij.inkwell.extra.PAPER";

    @Override
    protected boolean isSketch() {
        return true;
    }
}
