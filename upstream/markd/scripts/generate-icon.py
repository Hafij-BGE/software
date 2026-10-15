"""Generate the Markd app icon (PNG/ICO/ICNS) with Pillow.

Design: blue-to-violet squircle with a bold rounded "M" and the Markdown
down arrow. Run: python3 scripts/generate-icon.py
"""
import os
from PIL import Image, ImageDraw, ImageFilter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DESKTOP = os.path.join(ROOT, 'packages', 'desktop')
S = 2048  # supersampled canvas


def lerp(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def draw_icon():
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))

    # Squircle tile with a diagonal blue -> violet gradient.
    margin, radius = int(S * 0.07), int(S * 0.23)
    c0, c1 = (0x25, 0x63, 0xeb), (0x7c, 0x3a, 0xed)
    grad = Image.new('RGBA', (S, S))
    gd = ImageDraw.Draw(grad)
    for i in range(2 * S):
        gd.line([(i, 0), (0, i)], fill=lerp(c0, c1, i / (2 * S)) + (255,), width=2)
    mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle([margin, margin, S - margin, S - margin], radius, fill=255)

    shadow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(shadow).rounded_rectangle(
        [margin, margin + S * 0.02, S - margin, S - margin + S * 0.02], radius, fill=(20, 10, 60, 110))
    img = Image.alpha_composite(img, shadow.filter(ImageFilter.GaussianBlur(S * 0.02)))
    img.paste(grad, (0, 0), mask)

    # Soft light from the top-left corner.
    glow = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse([-S * 0.25, -S * 0.35, S * 0.75, S * 0.55], fill=(255, 255, 255, 30))
    glow = glow.filter(ImageFilter.GaussianBlur(S * 0.08))
    glow_masked = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    glow_masked.paste(glow, (0, 0), mask)
    img = Image.alpha_composite(img, glow_masked)

    d = ImageDraw.Draw(img)
    stroke = int(S * 0.085)

    def polyline(points, fill):
        d.line(points, fill=fill, width=stroke, joint='curve')
        r = stroke / 2
        for x, y in (points[0], points[-1]):
            d.ellipse([x - r, y - r, x + r, y + r], fill=fill)

    # Bold, round-capped "M".
    white = (255, 255, 255, 255)
    top, bottom = S * 0.33, S * 0.68
    polyline([(S * 0.225, bottom), (S * 0.225, top), (S * 0.38, S * 0.53),
              (S * 0.535, top), (S * 0.535, bottom)], white)

    # Markdown down-arrow in a light cyan accent.
    cyan = (0xa5, 0xf3, 0xfc, 255)
    ax = S * 0.735
    polyline([(ax, top), (ax, bottom)], cyan)
    polyline([(ax - S * 0.085, bottom - S * 0.085), (ax, bottom), (ax + S * 0.085, bottom - S * 0.085)], cyan)
    return img


def main():
    big = draw_icon()
    size = lambda n: big.resize((n, n), Image.LANCZOS)
    static = os.path.join(DESKTOP, 'static')
    icons = os.path.join(DESKTOP, 'build', 'icons')
    size(256).save(os.path.join(static, 'icon.png'))
    size(96).save(os.path.join(static, 'logo-96px.png'))
    size(1024).save(os.path.join(static, 'logo-small.png'))
    ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    size(256).save(os.path.join(static, 'icon.ico'), sizes=ico_sizes)
    size(1024).save(os.path.join(static, 'icon.icns'))
    size(512).save(os.path.join(icons, 'icon.png'))
    size(256).save(os.path.join(icons, 'icon.ico'), sizes=ico_sizes)
    size(1024).save(os.path.join(icons, 'icon.icns'))
    for n in (16, 24, 32, 48, 64, 128, 256, 512):
        folder = os.path.join(icons, f'{n}x{n}')
        if os.path.isdir(folder):
            size(n).save(os.path.join(folder, 'marktext.png'))
    docs = os.path.join(ROOT, 'docs', 'assets')
    if os.path.isdir(docs):
        size(256).save(os.path.join(docs, 'logo-small.png'))
    size(512).save(os.path.join(ROOT, 'docs', 'assets', 'markd-icon.png') if os.path.isdir(docs) else os.devnull, 'PNG')
    print('icons written')


if __name__ == '__main__':
    main()
