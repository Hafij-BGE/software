package com.hafij.inkwell.model;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * Line/dot geometry for Xournal++ solid backgrounds (ruled, lined, graph, dotted, staves,
 * isometric). New notes are plain white; these only show up in imported notes.
 */
public final class Template {
    public static final class Group {
        public final int color;
        public final float width;
        /** Lines as x1 y1 x2 y2 ..., or dot centres as x y ... */
        public final float[] coords;
        public final boolean dots;

        Group(int color, float width, List<Float> c, boolean dots) {
            this.color = color;
            this.width = width;
            this.dots = dots;
            coords = new float[c.size()];
            for (int i = 0; i < coords.length; i++) coords[i] = c.get(i);
        }
    }

    private static final int BLUE = 0xff40a0ff;
    private static final int RED = 0xffff0080;
    private static final int GRAY = 0xff909090;
    private static final float GRID = 14.17f;
    private static final Map<String, List<Group>> CACHE = new HashMap<>();

    private Template() {
    }

    public static synchronized List<Group> geometry(String style, String config, float w, float h) {
        String key = style + "|" + config + "|" + w + "|" + h;
        List<Group> g = CACHE.get(key);
        if (g == null) {
            g = build(style == null ? "plain" : style, parse(config), w, h);
            if (CACHE.size() > 32) CACHE.clear();
            CACHE.put(key, g);
        }
        return g;
    }

    private static Map<String, Float> parse(String config) {
        Map<String, Float> m = new HashMap<>();
        if (config == null) return m;
        for (String kv : config.split(",")) {
            String[] p = kv.split("=");
            if (p.length == 2) {
                try {
                    m.put(p[0].trim(), Float.parseFloat(p[1].trim()));
                } catch (NumberFormatException ignored) {
                    // skip
                }
            }
        }
        return m;
    }

    private static List<Group> build(String style, Map<String, Float> cfg, float w, float h) {
        List<Group> out = new ArrayList<>();
        List<Float> c = new ArrayList<>();
        switch (style) {
            case "ruled":
            case "lined": {
                for (float y = 80; y < h - 5; y += 24) add(c, 0, y, w, y);
                out.add(new Group(BLUE, 0.5f, c, false));
                if ("lined".equals(style)) {
                    float m1 = cfg.containsKey("m1") ? cfg.get("m1") : 72;
                    float x = m1 < 0 ? w + m1 : m1;
                    List<Float> m = new ArrayList<>();
                    add(m, x, 0, x, h);
                    out.add(new Group(RED, 0.5f, m, false));
                }
                break;
            }
            case "graph": {
                float margin = cfg.containsKey("m1") ? cfg.get("m1") : 0;
                float x0 = 0, y0 = 0, x1 = w, y1 = h;
                if (margin > 0) {
                    int nx = (int) ((w - 2 * margin) / GRID);
                    int ny = (int) ((h - 2 * margin) / GRID);
                    x0 = (w - nx * GRID) / 2;
                    y0 = (h - ny * GRID) / 2;
                    x1 = x0 + nx * GRID;
                    y1 = y0 + ny * GRID;
                }
                int bold = cfg.containsKey("bli") ? cfg.get("bli").intValue() : 0;
                List<Float> b = new ArrayList<>();
                int i = 0;
                for (float x = x0; x <= x1 + 0.01f; x += GRID, i++) add(bold > 0 && i % bold == 0 ? b : c, x, y0, x, y1);
                i = 0;
                for (float y = y0; y <= y1 + 0.01f; y += GRID, i++) add(bold > 0 && i % bold == 0 ? b : c, x0, y, x1, y);
                out.add(new Group(BLUE, 0.5f, c, false));
                if (!b.isEmpty()) out.add(new Group(BLUE, 1f, b, false));
                break;
            }
            case "dotted":
                for (float y = GRID; y < h; y += GRID) for (float x = GRID; x < w; x += GRID) {
                    c.add(x);
                    c.add(y);
                }
                out.add(new Group(BLUE, 1.5f, c, true));
                break;
            case "isodotted":
            case "isograph": {
                float rh = GRID * 0.8660254f;
                int row = 0;
                for (float y = rh; y < h; y += rh, row++) {
                    float off = row % 2 == 0 ? 0 : GRID / 2;
                    if ("isodotted".equals(style)) {
                        for (float x = off + GRID / 2; x < w; x += GRID) {
                            c.add(x);
                            c.add(y);
                        }
                    } else {
                        add(c, 0, y, w, y);
                    }
                }
                if ("isograph".equals(style)) {
                    float slope = (float) Math.tan(Math.toRadians(60));
                    float span = h / slope;
                    for (float x = -span; x < w + span; x += GRID) {
                        clipAdd(c, x, 0, x + span, h, w, h);
                        clipAdd(c, x, 0, x - span, h, w, h);
                    }
                }
                out.add(new Group(BLUE, "isodotted".equals(style) ? 1.5f : 0.5f, c, "isodotted".equals(style)));
                break;
            }
            case "staves": {
                for (float y = 60; y + 24 < h - 20; y += 54) {
                    for (int k = 0; k < 5; k++) add(c, 30, y + k * 6, w - 30, y + k * 6);
                }
                out.add(new Group(GRAY, 0.5f, c, false));
                break;
            }
            default:
                break;
        }
        return out;
    }

    private static void add(List<Float> c, float x1, float y1, float x2, float y2) {
        c.add(x1);
        c.add(y1);
        c.add(x2);
        c.add(y2);
    }

    /** Liang-Barsky clip of a segment to [0,w]x[0,h]. */
    private static void clipAdd(List<Float> c, float x1, float y1, float x2, float y2, float w, float h) {
        float t0 = 0, t1 = 1, dx = x2 - x1, dy = y2 - y1;
        float[] p = {-dx, dx, -dy, dy};
        float[] q = {x1, w - x1, y1, h - y1};
        for (int i = 0; i < 4; i++) {
            if (p[i] == 0) {
                if (q[i] < 0) return;
            } else {
                float t = q[i] / p[i];
                if (p[i] < 0) t0 = Math.max(t0, t);
                else t1 = Math.min(t1, t);
            }
        }
        if (t0 < t1) add(c, x1 + t0 * dx, y1 + t0 * dy, x1 + t1 * dx, y1 + t1 * dy);
    }
}
