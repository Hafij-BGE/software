#!/usr/bin/env python3
"""Draws the suite's icons for Word, Slide and Spreadsheet (the apps built from ArtCraft's
WordCraft, DeckCraft and GridCraft) into assets/hicolor/<size>/apps/craft-suite-<app>.png, with
an SVG of the same design in assets/hicolor/scalable/apps/.

Usage: python3 scripts/make-app-icons.py      (needs Pillow)

Each icon is a rounded square with a diagonal gradient in the app's colour and a white card
showing what the app makes: a page of text, a slide, a sheet of cells.
"""
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets" / "hicolor"
SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
# Drawn on a 1024 grid, then scaled down.
N = 1024

APPS = {
    # name: (gradient top-left, gradient bottom-right, accent on the card)
    "word": ("#5b8cff", "#2340c4", "#2f55d4"),
    "slide": ("#ffa04d", "#e04a17", "#e8601f"),
    "sheet": ("#3fd17f", "#0f8a47", "#14934c"),
}


def rgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def gradient(a, b):
    """A diagonal gradient from a (top left) to b (bottom right)."""
    a, b = rgb(a), rgb(b)
    small = Image.new("RGB", (256, 256))
    px = small.load()
    for y in range(256):
        for x in range(256):
            t = (x + y) / 510
            px[x, y] = tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))
    return small.resize((N, N), Image.BICUBIC)


def rounded_mask(box, radius):
    m = Image.new("L", (N, N), 0)
    ImageDraw.Draw(m).rounded_rectangle(box, radius, fill=255)
    return m


# The card and its contents per app, in 1024 units: (card box, card radius, shapes). Shapes are
# ("rect", box, radius, colour) with colour "accent", "soft" (accent, light) or "line" (grey).
def layout(app):
    if app == "word":
        card = (262, 196, 762, 828)
        shapes = [("rect", (330, 280, 600, 334), 27, "accent")]
        for i, w in enumerate([694, 660, 694, 560, 694, 620]):
            y = 400 + i * 66
            shapes.append(("rect", (330, y, w, y + 30), 15, "line"))
        return card, 56, shapes
    if app == "slide":
        card = (178, 270, 846, 700)
        shapes = [
            ("rect", (246, 340, 600, 396), 28, "accent"),
            ("rect", (246, 438, 520, 470), 16, "line"),
            ("rect", (246, 500, 480, 532), 16, "line"),
            ("circle", (590, 420, 778, 608), 0, "soft"),
            ("pie", (590, 420, 778, 608), 0, "accent"),
            # The stand under the slide.
            ("rect", (492, 700, 532, 792), 0, "white"),
            ("rect", (392, 780, 632, 822), 21, "white"),
        ]
        return card, 44, shapes
    card = (220, 220, 804, 804)
    shapes = [("rect", (220, 220, 804, 352), 0, "accent")]
    for x in (366, 512, 658):
        shapes.append(("rect", (x - 6, 220, x + 6, 804), 0, "line"))
    for y in (352, 465, 578, 691):
        shapes.append(("rect", (220, y - 6, 804, y + 6), 0, "line"))
    shapes.append(("rect", (220, 352, 366, 804), 0, "soft"))
    shapes.append(("rect", (520, 586, 650, 683), 0, "accent"))
    return card, 52, shapes


def colour_of(name, accent):
    a = rgb(accent)
    return {
        "accent": a + (255,),
        "soft": tuple(round(c + (255 - c) * 0.78) for c in a) + (255,),
        "line": (205, 211, 222, 255),
        "white": (255, 255, 255, 255),
    }[name]


def draw(app):
    top, bottom, accent = APPS[app]
    icon = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    # A soft shadow under the tile.
    shadow = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle((72, 92, 952, 972), 210, fill=(10, 20, 40, 90))
    icon.alpha_composite(shadow.filter(ImageFilter.GaussianBlur(28)))
    tile = gradient(top, bottom).convert("RGBA")
    icon.paste(tile, (0, 0), rounded_mask((64, 64, 960, 960), 210))
    # A highlight along the top.
    gloss = Image.new("RGBA", (N, N), (255, 255, 255, 0))
    ImageDraw.Draw(gloss).rounded_rectangle((64, 64, 960, 470), 210, fill=(255, 255, 255, 30))
    gloss = gloss.filter(ImageFilter.GaussianBlur(60))
    # Only on the tile.
    gloss.putalpha(Image.composite(gloss.getchannel("A"), Image.new("L", (N, N), 0), rounded_mask((64, 64, 960, 960), 210)))
    icon.alpha_composite(gloss)

    card, radius, shapes = layout(app)
    layer = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    cast = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    ImageDraw.Draw(cast).rounded_rectangle(tuple(v + (0, 18, 0, 18)[i] for i, v in enumerate(card)), radius, fill=(0, 0, 0, 70))
    layer.alpha_composite(cast.filter(ImageFilter.GaussianBlur(22)))
    d = ImageDraw.Draw(layer)
    d.rounded_rectangle(card, radius, fill=(255, 255, 255, 255))
    content = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    c = ImageDraw.Draw(content)
    for kind, box, r, col in shapes:
        fill = colour_of(col, accent)
        if kind == "rect":
            if r:
                c.rounded_rectangle(box, r, fill=fill)
            else:
                c.rectangle(box, fill=fill)
        elif kind == "circle":
            c.ellipse(box, fill=fill)
        elif kind == "pie":
            c.pieslice(box, -90, 30, fill=fill)
    # Card contents stay inside the card's rounded corners (the sheet's header row).
    clip = rounded_mask(card, radius)
    stand = Image.new("L", (N, N), 0)
    if app == "slide":
        ImageDraw.Draw(stand).rectangle((380, 700, 650, 830), fill=255)
    clip = Image.composite(Image.new("L", (N, N), 255), clip, stand)
    layer.paste(content, (0, 0), Image.composite(content.getchannel("A"), Image.new("L", (N, N), 0), clip))
    icon.alpha_composite(layer)
    return icon


def svg(app):
    top, bottom, accent = APPS[app]
    card, radius, shapes = layout(app)
    parts = [
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">',
        f'<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="{top}"/>'
        f'<stop offset="1" stop-color="{bottom}"/></linearGradient>'
        f'<clipPath id="c"><rect x="{card[0]}" y="{card[1]}" width="{card[2] - card[0]}" height="{card[3] - card[1]}" rx="{radius}"/></clipPath></defs>',
        '<rect x="64" y="64" width="896" height="896" rx="210" fill="url(#g)"/>',
        f'<rect x="{card[0]}" y="{card[1]}" width="{card[2] - card[0]}" height="{card[3] - card[1]}" rx="{radius}" fill="#fff"/>',
    ]
    for kind, box, r, col in shapes:
        fill = "#%02x%02x%02x" % colour_of(col, accent)[:3]
        x0, y0, x1, y1 = box
        clip = '' if (app == "slide" and y0 >= 700) else ' clip-path="url(#c)"'
        if kind == "rect":
            parts.append(f'<rect x="{x0}" y="{y0}" width="{x1 - x0}" height="{y1 - y0}" rx="{r}" fill="{fill}"{clip}/>')
        elif kind == "circle":
            parts.append(f'<circle cx="{(x0 + x1) / 2}" cy="{(y0 + y1) / 2}" r="{(x1 - x0) / 2}" fill="{fill}"/>')
        elif kind == "pie":
            cx, cy, rr = (x0 + x1) / 2, (y0 + y1) / 2, (x1 - x0) / 2
            # From 12 o'clock to 4 o'clock, clockwise.
            ex, ey = cx + rr * 0.8660254, cy + rr * 0.5
            parts.append(f'<path d="M{cx},{cy} L{cx},{cy - rr} A{rr},{rr} 0 0 1 {ex:.1f},{ey:.1f} Z" fill="{fill}"/>')
    parts.append("</svg>\n")
    return "\n".join(parts)


def main():
    for app in APPS:
        big = draw(app)
        for s in SIZES:
            d = OUT / f"{s}x{s}" / "apps"
            d.mkdir(parents=True, exist_ok=True)
            big.resize((s, s), Image.LANCZOS).save(d / f"craft-suite-{app}.png", optimize=True)
        d = OUT / "scalable" / "apps"
        d.mkdir(parents=True, exist_ok=True)
        (d / f"craft-suite-{app}.svg").write_text(svg(app))
        print(f"craft-suite-{app}")


if __name__ == "__main__":
    main()
