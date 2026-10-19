#!/usr/bin/env python3
"""Collects open-licence templates (CC0, CC-BY or public domain only), converts them with
LibreOffice to formats the suite's apps open, and lists them in templates/open.json with their
licence, author and source; build_index.py then draws their thumbnails, drops any the apps can't
open, and credits them in templates/ATTRIBUTION.md.

Sources (pinned):
- dohliam/libreoffice-impress-templates: community Impress templates, each collection's README
  giving every template's licence and author (CC0 from The Document Foundation's design team,
  CC-BY and public-domain ones from Apache OpenOffice, Fedora, KDE and others);
- "Business Document Templates" from extensions.libreoffice.org (CC-BY 4.0): Writer letters and a
  memo.

Needs LibreOffice (`soffice`). Run by .github/workflows/templates.yml.

Usage: scripts/templates/fetch_open.py [templates-dir]
"""

import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile

OUT = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "../../templates"))

IMPRESS_REPO = "dohliam/libreoffice-impress-templates"
IMPRESS_COMMIT = "7bdfa347c6fbdc784e788f1108f2df848af19d6e"
BUSINESS_URL = "https://extensions.libreoffice.org/assets/downloads/948/basic-business-templates-v2.zip"
BUSINESS_PAGE = "https://extensions.libreoffice.org/en/extensions/show/3881"

# Converted files bigger than this are left out (photographs in the background, mostly).
MAX_BYTES = 3 * 1024 * 1024
# Collection → category shown on the Home page.
CATEGORY = "Community"

entries = []


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "craft-suite-templates"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def licence(text, url):
    """The licence of a README table cell, as an SPDX-like name; None for ones not allowed here."""
    t = text.strip().lower().replace(" ", "")
    if t in ("cc0", "cc-0"):
        return "CC0-1.0"
    if t in ("publicdomain", "pd"):
        return "Public domain"
    if t == "cc-by":
        m = re.search(r"/by/(\d\.\d)", url or "")
        return f"CC-BY-{m.group(1)}" if m else "CC-BY"
    return None  # CC-BY-SA, CC-BY-NC, GPL, LGPL, BSD, MIT, Apache…: not included


def title_of(name):
    """"blue-curve" → "Blue Curve"; short names that are acronyms stay upper case."""
    upper = {"dna", "cs3", "oo", "kde", "lo"}
    return " ".join(w.upper() if w in upper else w.capitalize() for w in name.replace("_", "-").split("-") if w)


def soffice():
    for name in ("soffice", "libreoffice"):
        if shutil.which(name):
            return name
    sys.exit("LibreOffice (soffice) is needed")


def convert(src, fmt, outdir):
    """Converts `src` to `fmt` (pptx/docx) in `outdir`; the new file's path, or None."""
    r = subprocess.run([soffice(), "--headless", "--norestore", "--convert-to", fmt, "--outdir", outdir, src],
                       capture_output=True, text=True, timeout=300)
    out = os.path.join(outdir, os.path.splitext(os.path.basename(src))[0] + "." + fmt)
    if r.returncode != 0 or not os.path.isfile(out):
        print(f"  conversion failed: {src}: {r.stderr.strip()[:200]}")
        return None
    return out


def zip_odf(folder, dest):
    """An unpacked OpenDocument folder (as the repository keeps them) → one .otp file."""
    with zipfile.ZipFile(dest, "w") as z:
        mt = os.path.join(folder, "mimetype")
        if os.path.isfile(mt):
            z.write(mt, "mimetype", compress_type=zipfile.ZIP_STORED)
        for root, _, files in os.walk(folder):
            for f in sorted(files):
                p = os.path.join(root, f)
                rel = os.path.relpath(p, folder)
                if rel != "mimetype":
                    z.write(p, rel, compress_type=zipfile.ZIP_DEFLATED)


def keep(converted, sub, name, title, lic, author, source, origin):
    if os.path.getsize(converted) > MAX_BYTES:
        print(f"  too big, left out: {name} ({os.path.getsize(converted) // 1024} KB)")
        return
    dest_dir = os.path.join(OUT, sub)
    os.makedirs(dest_dir, exist_ok=True)
    ext = converted.rsplit(".", 1)[-1]
    dest = os.path.join(dest_dir, f"{name}.{ext}")
    shutil.move(converted, dest)
    entries.append({"file": os.path.relpath(dest, OUT), "title": title, "category": CATEGORY, "license": lic,
                    "author": author, "source": source, "origin": origin})


def impress(work):
    url = f"https://codeload.github.com/{IMPRESS_REPO}/tar.gz/{IMPRESS_COMMIT}"
    print(f"downloading {url}")
    with tarfile.open(fileobj=io.BytesIO(get(url)), mode="r:gz") as t:
        t.extractall(work, filter="data")
    root = os.path.join(work, f"libreoffice-impress-templates-{IMPRESS_COMMIT}")
    row = re.compile(r"^`([^`]+)`\s*\|\s*\[([^\]]+)\]\(([^)]*)\)\s*\|\s*([^|]*)\|")
    for collection in sorted(os.listdir(root)):
        readme = os.path.join(root, collection, "README.md")
        if not os.path.isfile(readme):
            continue
        with open(readme, encoding="utf-8") as fh:
            rows = [row.match(line.strip()) for line in fh]
        for m in filter(None, rows):
            name, lic_text, lic_url, author = m.group(1), m.group(2), m.group(3), m.group(4).strip()
            lic = licence(lic_text, lic_url)
            folder = os.path.join(root, collection, name)
            if not lic or not os.path.isdir(folder):
                continue
            author = re.sub(r"\[([^\]]+)\]\([^)]*\)", r"\1", author) or "Unknown"
            # An e-mail address isn't a name to show; credit the collection instead.
            if "@" in author:
                author = "LibreOffice Design Team contributor"
            otp = os.path.join(work, f"{collection}-{name}.otp")
            zip_odf(folder, otp)
            out = convert(otp, "pptx", work)
            if out:
                source = f"https://github.com/{IMPRESS_REPO}/tree/{IMPRESS_COMMIT}/{collection}/{name}"
                keep(out, "slide/community", f"{collection}-{name}", title_of(name), lic, author, source, f"{collection}/{name}")


def business(work):
    print(f"downloading {BUSINESS_URL}")
    data = get(BUSINESS_URL)
    d = os.path.join(work, "business")
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        z.extractall(d)
    # The download may hold the templates directly or inside an extension (.oxt).
    for root, _, files in os.walk(d):
        for f in files:
            if f.lower().endswith(".oxt"):
                with zipfile.ZipFile(os.path.join(root, f)) as z:
                    z.extractall(os.path.join(root, f + "-x"))
    for root, _, files in os.walk(d):
        for f in sorted(files):
            if not f.lower().endswith((".ott", ".odt")):
                continue
            out = convert(os.path.join(root, f), "docx", work)
            if out:
                stem = re.sub(r"[^a-z0-9]+", "-", os.path.splitext(f)[0].lower()).strip("-")
                keep(out, "word/community", f"business-{stem}", title_of(stem), "CC-BY-4.0",
                     "Business Document Templates contributors", BUSINESS_PAGE, "Business Document Templates")


def main():
    with tempfile.TemporaryDirectory() as work:
        impress(work)
        try:
            business(work)
        except Exception as e:  # one source down doesn't stop the rest
            print(f"business templates skipped: {e}")
    with open(os.path.join(OUT, "open.json"), "w", encoding="utf-8") as fh:
        json.dump({"templates": entries}, fh, indent=1, ensure_ascii=False)
        fh.write("\n")
    print(f"{len(entries)} open-licence templates")


if __name__ == "__main__":
    main()
