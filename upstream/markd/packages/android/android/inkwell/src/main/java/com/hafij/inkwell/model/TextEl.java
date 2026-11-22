package com.hafij.inkwell.model;

import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.graphics.Typeface;

/** A text box. (x, y) is the top-left corner, size is in points, like Xournal++. */
public class TextEl extends Element {
    public String font = "Sans";
    public float size = 12;
    public float x, y;
    public int color = 0xff000000;
    public String text = "";

    private static final Paint PAINT = new Paint(Paint.ANTI_ALIAS_FLAG);
    private transient RectF bounds;

    private Paint paint() {
        Paint p = PAINT;
        p.reset();
        p.setAntiAlias(true);
        p.setSubpixelText(true);
        p.setTextSize(size);
        p.setColor(color);
        String f = font == null ? "" : font.toLowerCase();
        int st = (f.contains("bold") ? Typeface.BOLD : 0) | (f.contains("italic") ? Typeface.ITALIC : 0);
        Typeface base = f.contains("mono") ? Typeface.MONOSPACE : f.contains("serif") && !f.contains("sans") ? Typeface.SERIF : Typeface.SANS_SERIF;
        p.setTypeface(Typeface.create(base, st));
        return p;
    }

    /** {ascent (positive), line height} in points, matching how draw() lays out lines. */
    public float[] metrics() {
        Paint.FontMetrics fm = paint().getFontMetrics();
        return new float[]{-fm.ascent, fm.descent - fm.ascent};
    }

    private float lineHeight(Paint p) {
        Paint.FontMetrics fm = p.getFontMetrics();
        return fm.descent - fm.ascent;
    }

    public void invalidate() {
        bounds = null;
    }

    @Override
    public RectF bounds() {
        if (bounds == null) {
            Paint p = paint();
            String[] lines = text.split("\n", -1);
            float w = 0;
            for (String l : lines) w = Math.max(w, p.measureText(l));
            bounds = new RectF(x, y, x + Math.max(w, size / 2), y + lines.length * lineHeight(p));
        }
        return bounds;
    }

    @Override
    public void draw(Canvas c) {
        Paint p = paint();
        Paint.FontMetrics fm = p.getFontMetrics();
        float lh = fm.descent - fm.ascent;
        float yy = y - fm.ascent;
        for (String l : text.split("\n", -1)) {
            c.drawText(l, x, yy, p);
            yy += lh;
        }
    }

    @Override
    public void translate(float dx, float dy) {
        x += dx;
        y += dy;
        invalidate();
    }

    @Override
    public void scale(float sx, float sy, float px, float py) {
        x = px + (x - px) * sx;
        y = py + (y - py) * sy;
        size *= (float) Math.sqrt(Math.abs(sx * sy));
        invalidate();
    }

    @Override
    public Element copy() {
        TextEl t = new TextEl();
        t.font = font;
        t.size = size;
        t.x = x;
        t.y = y;
        t.color = color;
        t.text = text;
        return t;
    }

    @Override
    public boolean hit(float px, float py, float radius) {
        RectF b = bounds();
        return px >= b.left - radius && px <= b.right + radius && py >= b.top - radius && py <= b.bottom + radius;
    }

    @Override
    public int getColor() {
        return color;
    }

    @Override
    public void setColor(int argb) {
        color = argb;
    }
}
