#!/usr/bin/env python3
"""Convert the stroke-style Lucide SVG icons shipped with Xournal++ into Android
VectorDrawables. Colours are dropped; the drawable is tinted by the theme.

usage: svg2vector.py <svg-dir> <res/drawable dir>
"""
import os
import re
import sys
import xml.etree.ElementTree as ET

NUM = r'[-+]?(?:\d+\.?\d*|\.\d+)(?:[eE][-+]?\d+)?'
ARGS = {'M': 2, 'L': 2, 'H': 1, 'V': 1, 'C': 6, 'S': 4, 'Q': 4, 'T': 2, 'A': 7, 'Z': 0}


def normalize_path(d):
    """Re-emit path data with explicit separators (Android's parser is picky
    about packed arc flags such as 'a2 2 0 011 1')."""
    out = []
    i = 0
    cmd = None
    n = len(d)
    while i < n:
        c = d[i]
        if c.isspace() or c == ',':
            i += 1
            continue
        if c.isalpha():
            cmd = c
            out.append(c)
            i += 1
            if c.upper() == 'Z':
                cmd = None
            continue
        if cmd is None:
            raise ValueError('number without command')
        count = ARGS[cmd.upper()]
        vals = []
        for k in range(count):
            while i < n and (d[i].isspace() or d[i] == ','):
                i += 1
            if cmd.upper() == 'A' and k in (3, 4):
                vals.append(d[i])
                i += 1
                continue
            m = re.compile(NUM).match(d, i)
            if not m:
                raise ValueError('bad number at %d in %r' % (i, d))
            vals.append(m.group(0))
            i = m.end()
        out.append(' '.join(vals))
        if cmd == 'M':
            cmd = 'L'
        elif cmd == 'm':
            cmd = 'l'
    return ' '.join(out)


def circle(cx, cy, r):
    return 'M %s %s m %s 0 a %s %s 0 1 0 %s 0 a %s %s 0 1 0 %s 0' % (
        cx, cy, -r, r, r, 2 * r, r, r, -2 * r)


def ellipse(cx, cy, rx, ry):
    return 'M %s %s m %s 0 a %s %s 0 1 0 %s 0 a %s %s 0 1 0 %s 0' % (
        cx, cy, -rx, rx, ry, 2 * rx, rx, ry, -2 * rx)


def rect(x, y, w, h, rx):
    if rx <= 0:
        return 'M %s %s h %s v %s h %s Z' % (x, y, w, h, -w)
    rx = min(rx, w / 2, h / 2)
    return ('M %s %s h %s a %s %s 0 0 1 %s %s v %s a %s %s 0 0 1 %s %s h %s '
            'a %s %s 0 0 1 %s %s v %s a %s %s 0 0 1 %s %s Z') % (
        x + rx, y, w - 2 * rx, rx, rx, rx, rx, h - 2 * rx, rx, rx, -rx, rx,
        -(w - 2 * rx), rx, rx, -rx, -rx, -(h - 2 * rx), rx, rx, rx, -rx)


def style_of(el, inherited):
    st = dict(inherited)
    for k in ('fill', 'stroke', 'stroke-width', 'stroke-linecap', 'stroke-linejoin'):
        if el.get(k) is not None:
            st[k] = el.get(k)
    for part in (el.get('style') or '').split(';'):
        if ':' in part:
            k, v = part.split(':', 1)
            st[k.strip()] = v.strip()
    return st


def f(el, name, default=0.0):
    v = el.get(name)
    return float(v) if v is not None else default


def convert(svg_path):
    tree = ET.parse(svg_path)
    root = tree.getroot()
    ns = '{http://www.w3.org/2000/svg}'
    base = style_of(root, {'fill': 'black', 'stroke': 'none', 'stroke-width': '1'})
    paths = []

    def walk(node, st):
        for el in node:
            tag = el.tag.replace(ns, '')
            if tag in ('defs', 'metadata', 'title') or tag.startswith('{'):
                continue
            if el.get('transform'):
                raise ValueError('transform unsupported')
            s = style_of(el, st)
            if tag == 'g':
                walk(el, s)
                continue
            if tag == 'path':
                d = normalize_path(el.get('d'))
            elif tag == 'circle':
                d = circle(f(el, 'cx'), f(el, 'cy'), f(el, 'r'))
            elif tag == 'ellipse':
                d = ellipse(f(el, 'cx'), f(el, 'cy'), f(el, 'rx'), f(el, 'ry'))
            elif tag == 'rect':
                d = rect(f(el, 'x'), f(el, 'y'), f(el, 'width'), f(el, 'height'),
                         f(el, 'rx', f(el, 'ry')))
            elif tag == 'line':
                d = 'M %s %s L %s %s' % (f(el, 'x1'), f(el, 'y1'), f(el, 'x2'), f(el, 'y2'))
            elif tag in ('polyline', 'polygon'):
                pts = re.findall(NUM, el.get('points'))
                d = 'M ' + ' L '.join('%s %s' % (pts[k], pts[k + 1]) for k in range(0, len(pts), 2))
                if tag == 'polygon':
                    d += ' Z'
            else:
                raise ValueError('element %s unsupported' % tag)
            if 'url(' in s.get('fill', '') or 'url(' in s.get('stroke', ''):
                raise ValueError('gradient unsupported')
            attrs = ['android:pathData="%s"' % d]
            if s.get('fill', 'none') != 'none':
                attrs.append('android:fillColor="#FFFFFFFF"')
            if s.get('stroke', 'none') != 'none':
                attrs.append('android:strokeColor="#FFFFFFFF"')
                attrs.append('android:strokeWidth="%s"' % s.get('stroke-width', '1'))
                attrs.append('android:strokeLineCap="%s"' % s.get('stroke-linecap', 'butt'))
                attrs.append('android:strokeLineJoin="%s"' % s.get('stroke-linejoin', 'miter'))
            paths.append('    <path ' + '\n        '.join(attrs) + ' />')

    walk(root, base)
    vb = (root.get('viewBox') or '0 0 %s %s' % (root.get('width', 24), root.get('height', 24))).split()
    return ('<?xml version="1.0" encoding="utf-8"?>\n'
            '<!-- Lucide icon (ISC licence), as shipped with Xournal++. -->\n'
            '<vector xmlns:android="http://schemas.android.com/apk/res/android"\n'
            '    android:width="24dp" android:height="24dp"\n'
            '    android:viewportWidth="%s" android:viewportHeight="%s"\n'
            '    android:tint="?android:attr/colorControlNormal">\n%s\n</vector>\n') % (
        vb[2], vb[3], '\n'.join(paths))


def main():
    src, dst = sys.argv[1], sys.argv[2]
    ok = skipped = 0
    for name in sorted(os.listdir(src)):
        if not name.endswith('.svg'):
            continue
        out = 'ic_' + name[len('xopp-'):-4].replace('-', '_') + '.xml'
        try:
            xml = convert(os.path.join(src, name))
        except Exception as e:  # noqa: BLE001
            print('skip %s: %s' % (name, e))
            skipped += 1
            continue
        with open(os.path.join(dst, out), 'w') as fh:
            fh.write(xml)
        ok += 1
    print('converted %d, skipped %d' % (ok, skipped))


if __name__ == '__main__':
    main()
