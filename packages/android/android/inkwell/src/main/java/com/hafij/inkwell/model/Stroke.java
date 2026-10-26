package com.hafij.inkwell.model;

import android.graphics.Canvas;
import android.graphics.DashPathEffect;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PathEffect;
import android.graphics.RectF;

import java.util.Arrays;

/** A pen, highlighter or whiteout stroke, stored the way Xournal++ stores it. */
public class Stroke extends Element {
    public static final String PEN = "pen";
    public static final String HIGHLIGHTER = "highlighter";
    public static final String ERASER = "eraser";

    public String tool = PEN;
    public int color = 0xff000000;
    public float width = 1.41f;
    /** Optional per-point widths (pressure), same length as the point count. */
    public float[] pressures;
    /** x0 y0 x1 y1 ... */
    public float[] pts = new float[0];
    /** -1 = no fill, else fill alpha 0..255. */
    public int fill = -1;
    public String cap = "round";
    /** null = solid, "dash", "dashdot", "dot" or "cust: a b ...". */
    public String style;

    private transient Path path;
    private transient RectF bounds;
    private static final Paint PAINT = new Paint(Paint.ANTI_ALIAS_FLAG);

    public int count() {
        return pts.length / 2;
    }

    public void invalidate() {
        path = null;
        bounds = null;
    }

    public void setPoints(float[] p, float[] pr) {
        pts = p;
        pressures = pr;
        invalidate();
    }

    private Path path() {
        if (path == null) {
            Path p = new Path();
            int n = count();
            if (n > 0) {
                p.moveTo(pts[0], pts[1]);
                if (n == 1) {
                    p.lineTo(pts[0] + 0.01f, pts[1]);
                }
                for (int i = 1; i < n; i++) {
                    p.lineTo(pts[2 * i], pts[2 * i + 1]);
                }
            }
            path = p;
        }
        return path;
    }

    @Override
    public RectF bounds() {
        if (bounds == null) {
            float minX = Float.MAX_VALUE, minY = Float.MAX_VALUE, maxX = -Float.MAX_VALUE, maxY = -Float.MAX_VALUE;
            for (int i = 0; i + 1 < pts.length; i += 2) {
                minX = Math.min(minX, pts[i]);
                maxX = Math.max(maxX, pts[i]);
                minY = Math.min(minY, pts[i + 1]);
                maxY = Math.max(maxY, pts[i + 1]);
            }
            float w = maxWidth() / 2f;
            bounds = pts.length == 0 ? new RectF() : new RectF(minX - w, minY - w, maxX + w, maxY + w);
        }
        return bounds;
    }

    public float maxWidth() {
        float w = width;
        if (pressures != null) {
            for (float v : pressures) w = Math.max(w, v);
        }
        return w;
    }

    private PathEffect effect(float w) {
        float[] d = dashPattern(w);
        return d == null ? null : new DashPathEffect(d, 0);
    }

    /** Dash on/off lengths in points for this stroke's style, or null when solid. */
    public float[] dashPattern(float w) {
        if (style == null || style.isEmpty()) return null;
        float[] d;
        switch (style) {
            case "dash":
            case "dashed":
                d = new float[]{6, 3};
                break;
            case "dashdot":
                d = new float[]{6, 3, 0.5f, 3};
                break;
            case "dot":
            case "dotted":
                d = new float[]{0.5f, 3};
                break;
            default:
                if (style.startsWith("cust:")) {
                    String[] parts = style.substring(5).trim().split("\\s+");
                    d = new float[parts.length % 2 == 0 ? parts.length : parts.length * 2];
                    try {
                        for (int i = 0; i < d.length; i++) d[i] = Float.parseFloat(parts[i % parts.length]);
                    } catch (NumberFormatException e) {
                        return null;
                    }
                } else {
                    return null;
                }
        }
        // Dash lengths scale with the line width, as in Xournal++.
        float s = Math.max(w, 0.5f);
        float[] scaled = new float[d.length];
        for (int i = 0; i < d.length; i++) scaled[i] = Math.max(d[i] * s, 0.01f);
        return scaled;
    }

    @Override
    public void draw(Canvas c) {
        int n = count();
        if (n == 0) return;
        Paint p = PAINT;
        p.reset();
        p.setAntiAlias(true);
        p.setStrokeJoin(Paint.Join.ROUND);
        p.setStrokeCap("butt".equals(cap) ? Paint.Cap.BUTT : "square".equals(cap) ? Paint.Cap.SQUARE : Paint.Cap.ROUND);
        int col = ERASER.equals(tool) ? 0xffffffff : color;
        if (fill >= 0 && n > 2) {
            p.setStyle(Paint.Style.FILL);
            p.setColor(col);
            int a = Color.alpha(col) * fill / 255;
            p.setAlpha(HIGHLIGHTER.equals(tool) ? Math.min(a, Color.alpha(col)) : a);
            c.drawPath(path(), p);
        }
        p.setStyle(Paint.Style.STROKE);
        p.setColor(col);
        if (pressures == null || pressures.length != n || style != null) {
            p.setStrokeWidth(width);
            p.setPathEffect(effect(width));
            c.drawPath(path(), p);
            p.setPathEffect(null);
            return;
        }
        if (Color.alpha(col) < 255) {
            // Segments would overlap and darken; draw an averaged single path.
            float sum = 0;
            for (float v : pressures) sum += v;
            p.setStrokeWidth(sum / n);
            c.drawPath(path(), p);
            return;
        }
        for (int i = 1; i < n; i++) {
            p.setStrokeWidth((pressures[i - 1] + pressures[i]) / 2f);
            c.drawLine(pts[2 * i - 2], pts[2 * i - 1], pts[2 * i], pts[2 * i + 1], p);
        }
        if (n == 1) {
            p.setStrokeWidth(pressures[0]);
            c.drawPoint(pts[0], pts[1], p);
        }
    }

    @Override
    public void translate(float dx, float dy) {
        for (int i = 0; i + 1 < pts.length; i += 2) {
            pts[i] += dx;
            pts[i + 1] += dy;
        }
        invalidate();
    }

    @Override
    public void scale(float sx, float sy, float px, float py) {
        for (int i = 0; i + 1 < pts.length; i += 2) {
            pts[i] = px + (pts[i] - px) * sx;
            pts[i + 1] = py + (pts[i + 1] - py) * sy;
        }
        float f = (float) Math.sqrt(Math.abs(sx * sy));
        width *= f;
        if (pressures != null) for (int i = 0; i < pressures.length; i++) pressures[i] *= f;
        invalidate();
    }

    @Override
    public Element copy() {
        Stroke s = new Stroke();
        s.tool = tool;
        s.color = color;
        s.width = width;
        s.pressures = pressures == null ? null : Arrays.copyOf(pressures, pressures.length);
        s.pts = Arrays.copyOf(pts, pts.length);
        s.fill = fill;
        s.cap = cap;
        s.style = style;
        return s;
    }

    @Override
    public boolean hit(float x, float y, float radius) {
        RectF b = bounds();
        if (x < b.left - radius || x > b.right + radius || y < b.top - radius || y > b.bottom + radius) return false;
        float r = radius + width / 2f;
        int n = count();
        if (n == 1) return dist2(x, y, pts[0], pts[1]) <= r * r;
        for (int i = 1; i < n; i++) {
            if (segDist2(x, y, pts[2 * i - 2], pts[2 * i - 1], pts[2 * i], pts[2 * i + 1]) <= r * r) return true;
        }
        return false;
    }

    /** Index of every point within radius of (x, y). */
    public boolean[] pointsNear(float x, float y, float radius) {
        int n = count();
        boolean[] near = new boolean[n];
        float r = radius + width / 2f;
        for (int i = 0; i < n; i++) near[i] = dist2(x, y, pts[2 * i], pts[2 * i + 1]) <= r * r;
        // Also catch long segments that pass under the eraser between two points.
        for (int i = 1; i < n; i++) {
            if (!near[i - 1] && !near[i]
                    && segDist2(x, y, pts[2 * i - 2], pts[2 * i - 1], pts[2 * i], pts[2 * i + 1]) <= r * r) {
                near[i] = true;
            }
        }
        return near;
    }

    static float dist2(float x, float y, float ax, float ay) {
        float dx = x - ax, dy = y - ay;
        return dx * dx + dy * dy;
    }

    static float segDist2(float x, float y, float ax, float ay, float bx, float by) {
        float vx = bx - ax, vy = by - ay;
        float len = vx * vx + vy * vy;
        float t = len == 0 ? 0 : ((x - ax) * vx + (y - ay) * vy) / len;
        t = Math.max(0, Math.min(1, t));
        return dist2(x, y, ax + t * vx, ay + t * vy);
    }

    /** Copy of points [from, to) as a new stroke with the same look. */
    public Stroke slice(int from, int to) {
        Stroke s = (Stroke) copy();
        s.pts = Arrays.copyOfRange(pts, 2 * from, 2 * to);
        s.pressures = pressures == null || pressures.length != count() ? null : Arrays.copyOfRange(pressures, from, to);
        s.invalidate();
        return s;
    }

    @Override
    public int getColor() {
        return color;
    }

    @Override
    public void setColor(int argb) {
        if (HIGHLIGHTER.equals(tool)) {
            color = (argb & 0x00ffffff) | (color & 0xff000000);
        } else {
            color = argb;
        }
    }

    /** Small helper so the model does not depend on android.graphics.Color statics in tests. */
    static final class Color {
        static int alpha(int c) {
            return c >>> 24;
        }
    }
}
