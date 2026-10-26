package com.hafij.inkwell.editor;

import com.hafij.inkwell.model.Stroke;

/** Builds shape strokes and recognises hand-drawn lines, rectangles and ellipses. */
public final class Shapes {
    public static final int LINE = 0, RECT = 1, ELLIPSE = 2, ARROW = 3, DOUBLE_ARROW = 4, AXES = 5;

    private Shapes() {
    }

    /** Points for a shape dragged from (x0, y0) to (x1, y1). Arrows use pen-up-free paths. */
    public static float[] build(int type, float x0, float y0, float x1, float y1, float width) {
        switch (type) {
            case RECT:
                return new float[]{x0, y0, x1, y0, x1, y1, x0, y1, x0, y0};
            case ELLIPSE:
                return ellipse((x0 + x1) / 2, (y0 + y1) / 2, Math.abs(x1 - x0) / 2, Math.abs(y1 - y0) / 2);
            case ARROW:
                return arrow(x0, y0, x1, y1, width, false);
            case DOUBLE_ARROW:
                return arrow(x0, y0, x1, y1, width, true);
            case AXES:
                // y axis down to the origin, then the x axis.
                return new float[]{x0, y0, x0, y1, x1, y1};
            default:
                return new float[]{x0, y0, x1, y1};
        }
    }

    static float[] ellipse(float cx, float cy, float rx, float ry) {
        int n = 64;
        float[] p = new float[(n + 1) * 2];
        for (int i = 0; i <= n; i++) {
            double a = 2 * Math.PI * i / n;
            p[2 * i] = cx + (float) (rx * Math.cos(a));
            p[2 * i + 1] = cy + (float) (ry * Math.sin(a));
        }
        return p;
    }

    private static float[] arrow(float x0, float y0, float x1, float y1, float width, boolean both) {
        float dx = x1 - x0, dy = y1 - y0;
        float len = (float) Math.hypot(dx, dy);
        if (len < 0.01f) return new float[]{x0, y0, x1, y1};
        float ux = dx / len, uy = dy / len;
        float head = Math.min(len / 3, Math.max(8, width * 6));
        float hx = head * 0.866f, hy = head * 0.5f;
        // Head at the end: shaft, then one barb out and back, then the other barb.
        float ax = x1 - ux * hx - uy * hy, ay = y1 - uy * hx + ux * hy;
        float bx = x1 - ux * hx + uy * hy, by = y1 - uy * hx - ux * hy;
        if (!both) return new float[]{x0, y0, x1, y1, ax, ay, x1, y1, bx, by};
        float cx = x0 + ux * hx - uy * hy, cy = y0 + uy * hx + ux * hy;
        float ex = x0 + ux * hx + uy * hy, ey = y0 + uy * hx - ux * hy;
        return new float[]{cx, cy, x0, y0, ex, ey, x0, y0, x1, y1, ax, ay, x1, y1, bx, by};
    }

    /** Returns replacement points if the stroke looks like a line, rectangle or ellipse. */
    public static float[] recognize(Stroke s) {
        int n = s.count();
        if (n < 4) return null;
        float[] p = s.pts;
        float x0 = p[0], y0 = p[1], xn = p[2 * n - 2], yn = p[2 * n - 1];
        float chord = (float) Math.hypot(xn - x0, yn - y0);
        float length = 0;
        for (int i = 1; i < n; i++) length += (float) Math.hypot(p[2 * i] - p[2 * i - 2], p[2 * i + 1] - p[2 * i - 1]);
        if (length < 10) return null;

        // Straight line: the path barely leaves the chord.
        if (chord > 0.9f * length) return new float[]{x0, y0, xn, yn};

        // Closed shapes: end comes back near the start.
        if (chord > 0.2f * length) return null;
        float minX = Float.MAX_VALUE, minY = Float.MAX_VALUE, maxX = -Float.MAX_VALUE, maxY = -Float.MAX_VALUE;
        for (int i = 0; i < n; i++) {
            minX = Math.min(minX, p[2 * i]);
            maxX = Math.max(maxX, p[2 * i]);
            minY = Math.min(minY, p[2 * i + 1]);
            maxY = Math.max(maxY, p[2 * i + 1]);
        }
        float w = maxX - minX, h = maxY - minY;
        if (w < 5 || h < 5) return null;
        float cx = (minX + maxX) / 2, cy = (minY + maxY) / 2, rx = w / 2, ry = h / 2;
        double ellipseErr = 0, rectErr = 0;
        for (int i = 0; i < n; i++) {
            float x = p[2 * i] - cx, y = p[2 * i + 1] - cy;
            double r = Math.sqrt((x * x) / (rx * rx) + (y * y) / (ry * ry));
            ellipseErr += Math.abs(r - 1);
            float d = Math.min(Math.min(Math.abs(p[2 * i] - minX), Math.abs(p[2 * i] - maxX)) / rx,
                    Math.min(Math.abs(p[2 * i + 1] - minY), Math.abs(p[2 * i + 1] - maxY)) / ry);
            rectErr += d;
        }
        ellipseErr /= n;
        rectErr /= n;
        if (rectErr < 0.08 && rectErr < ellipseErr) {
            return new float[]{minX, minY, maxX, minY, maxX, maxY, minX, maxY, minX, minY};
        }
        if (ellipseErr < 0.12) return ellipse(cx, cy, rx, ry);
        return null;
    }
}
