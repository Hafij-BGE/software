"""Small writers for Word (.docx), Excel (.xlsx) and PowerPoint (.pptx) files: enough to build
Craft Suite's own templates (make_own.py) with nothing but Python's standard library.

Every file is written with fixed timestamps, so the same input gives the same bytes.
"""

import zipfile
from xml.sax.saxutils import escape

EPOCH = (2026, 1, 1, 0, 0, 0)


def esc(text):
    return escape(str(text), {'"': "&quot;"})


def write_zip(path, parts):
    """Writes `parts` ({name: str or bytes}) as a zip, [Content_Types].xml first."""
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for name in sorted(parts, key=lambda n: (n != "[Content_Types].xml", n)):
            data = parts[name]
            info = zipfile.ZipInfo(name, EPOCH)
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data.encode("utf-8") if isinstance(data, str) else data)


XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n'
REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL = "http://schemas.openxmlformats.org/package/2006/relationships"


def rels(items):
    """items: [(id, type suffix or full type URL, target)]."""
    out = [XML, f'<Relationships xmlns="{PKG_REL}">']
    for rid, kind, target in items:
        t = kind if kind.startswith("http") else f"{REL}/{kind}"
        out.append(f'<Relationship Id="{rid}" Type="{t}" Target="{esc(target)}"/>')
    out.append("</Relationships>")
    return "".join(out)


def core_props(title, subject=""):
    return (
        XML + '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
        'xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" '
        'xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">'
        f"<dc:title>{esc(title)}</dc:title><dc:subject>{esc(subject)}</dc:subject>"
        "<dc:creator>Craft Suite</dc:creator><cp:keywords>template</cp:keywords>"
        '<dcterms:created xsi:type="dcterms:W3CDTF">2026-01-01T00:00:00Z</dcterms:created>'
        "</cp:coreProperties>"
    )


def app_props(app):
    return (
        XML + '<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">'
        f"<Application>{esc(app)}</Application></Properties>"
    )


CORE_REL = "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties"
APP_REL = "extended-properties"


# ---------------------------------------------------------------- Word


class Docx:
    """A document built paragraph by paragraph. Colours are hex "RRGGBB"; sizes are points."""

    def __init__(self, title, body_font="Source Sans 3", heading_font="Inter", accent="2B579A", text="222222",
                 body_size=11, page="A4", margins=1440):
        self.title = title
        self.body_font, self.heading_font = body_font, heading_font
        self.accent, self.text = accent, text
        self.body_size = body_size
        self.page, self.margins = page, margins
        self.body = []

    # -- runs and paragraphs

    def _run(self, run):
        if isinstance(run, str):
            run = (run, {})
        text, f = run
        props = []
        if f.get("font"):
            fn = f["font"]
            props.append(f'<w:rFonts w:ascii="{fn}" w:hAnsi="{fn}" w:cs="{fn}"/>')
        if f.get("bold"):
            props.append("<w:b/>")
        if f.get("italic"):
            props.append("<w:i/>")
        if f.get("caps"):
            props.append("<w:caps/>")
        if f.get("color"):
            props.append(f'<w:color w:val="{f["color"]}"/>')
        if f.get("spacing"):
            props.append(f'<w:spacing w:val="{int(f["spacing"] * 20)}"/>')
        if f.get("size"):
            props.append(f'<w:sz w:val="{int(f["size"] * 2)}"/><w:szCs w:val="{int(f["size"] * 2)}"/>')
        rpr = f"<w:rPr>{''.join(props)}</w:rPr>" if props else ""
        out = []
        for i, line in enumerate(str(text).split("\n")):
            if i:
                out.append(f"<w:r>{rpr}<w:br/></w:r>")
            out.append(f'<w:r>{rpr}<w:t xml:space="preserve">{esc(line)}</w:t></w:r>')
        return "".join(out)

    def _para(self, runs, style=None, align=None, before=None, after=None, line=None, shade=None, border=None,
              indent=None, num=None, keep_next=False, page_break_before=False, tabs=None):
        ppr = []
        if style:
            ppr.append(f'<w:pStyle w:val="{style}"/>')
        if keep_next:
            ppr.append("<w:keepNext/>")
        if page_break_before:
            ppr.append("<w:pageBreakBefore/>")
        if num is not None:
            ppr.append(f'<w:numPr><w:ilvl w:val="0"/><w:numId w:val="{num}"/></w:numPr>')
        if border:
            side, size, color, space = border
            ppr.append(f'<w:pBdr><w:{side} w:val="single" w:sz="{size}" w:space="{space}" w:color="{color}"/></w:pBdr>')
        if shade:
            ppr.append(f'<w:shd w:val="clear" w:color="auto" w:fill="{shade}"/>')
        if tabs:
            ppr.append("<w:tabs>" + "".join(f'<w:tab w:val="{k}" w:pos="{p}"/>' for k, p in tabs) + "</w:tabs>")
        if before is not None or after is not None or line is not None:
            a = []
            if before is not None:
                a.append(f'w:before="{int(before * 20)}"')
            if after is not None:
                a.append(f'w:after="{int(after * 20)}"')
            if line is not None:
                a.append(f'w:line="{int(line * 240)}" w:lineRule="auto"')
            ppr.append(f"<w:spacing {' '.join(a)}/>")
        if indent:
            ppr.append(f'<w:ind w:left="{indent}"/>')
        if align:
            ppr.append(f'<w:jc w:val="{align}"/>')
        if isinstance(runs, (str, tuple)):
            runs = [runs]
        body = "".join(self._run(r) for r in runs)
        return f"<w:p><w:pPr>{''.join(ppr)}</w:pPr>{body}</w:p>"

    def p(self, runs="", **kw):
        self.body.append(self._para(runs, **kw))

    def heading(self, text, level=1, **kw):
        self.body.append(self._para(text, style=f"Heading{level}", keep_next=True, **kw))

    def title_(self, text, **kw):
        self.body.append(self._para(text, style="Title", **kw))

    def subtitle(self, text, **kw):
        self.body.append(self._para(text, style="Subtitle", **kw))

    def bullet(self, runs, **kw):
        self.body.append(self._para(runs, style="ListParagraph", num=1, after=3, **kw))

    def number(self, runs, **kw):
        self.body.append(self._para(runs, style="ListParagraph", num=2, after=3, **kw))

    def page_break(self):
        self.body.append('<w:p><w:r><w:br w:type="page"/></w:r></w:p>')

    def table(self, rows, widths, header_fill=None, header_color="FFFFFF", zebra=None, border="D0D5DD",
              cell_pad=100, fills=None, aligns=None, bold_first_col=False, font_size=None, no_borders=False):
        """rows: lists of cell contents (str, run tuple, or list of paragraphs given as run lists)."""
        tw = sum(widths)
        b = "nil" if no_borders else "single"
        bd = "".join(f'<w:{s} w:val="{b}" w:sz="4" w:space="0" w:color="{border}"/>'
                     for s in ("top", "left", "bottom", "right", "insideH", "insideV"))
        out = [f'<w:tbl><w:tblPr><w:tblW w:w="{tw}" w:type="dxa"/><w:tblLayout w:type="fixed"/>'
               f"<w:tblBorders>{bd}</w:tblBorders>"
               f'<w:tblCellMar><w:top w:w="{cell_pad}" w:type="dxa"/><w:left w:w="{cell_pad + 20}" w:type="dxa"/>'
               f'<w:bottom w:w="{cell_pad}" w:type="dxa"/><w:right w:w="{cell_pad + 20}" w:type="dxa"/></w:tblCellMar>'
               "</w:tblPr><w:tblGrid>"]
        out += [f'<w:gridCol w:w="{w}"/>' for w in widths]
        out.append("</w:tblGrid>")
        for ri, row in enumerate(rows):
            header = ri == 0 and header_fill is not None
            out.append("<w:tr>" + ('<w:trPr><w:tblHeader/></w:trPr>' if header else ""))
            for ci, cell in enumerate(row):
                fill = None
                if fills and (ri, ci) in fills:
                    fill = fills[(ri, ci)]
                elif header:
                    fill = header_fill
                elif zebra and ri % 2 == 0:
                    fill = zebra
                tc = [f'<w:tcPr><w:tcW w:w="{widths[ci]}" w:type="dxa"/>']
                if fill:
                    tc.append(f'<w:shd w:val="clear" w:color="auto" w:fill="{fill}"/>')
                tc.append('<w:vAlign w:val="center"/></w:tcPr>')
                paras = cell if isinstance(cell, list) else [cell]
                align = aligns[ci] if aligns else None
                for para in paras:
                    runs = para if isinstance(para, list) else [para]
                    fmt = {}
                    if header:
                        fmt = {"bold": True, "color": header_color}
                    elif bold_first_col and ci == 0:
                        fmt = {"bold": True}
                    if font_size:
                        fmt["size"] = font_size
                    runs = [(r, dict(fmt)) if isinstance(r, str) else (r[0], {**fmt, **r[1]}) for r in runs]
                    tc.append(self._para(runs, after=0, align=align))
                out.append("<w:tc>" + "".join(tc) + "</w:tc>")
            out.append("</w:tr>")
        out.append("</w:tbl>")
        self.body.append("".join(out))
        self.body.append(self._para("", after=4))

    # -- parts

    def _styles(self):
        a, t, bf, hf = self.accent, self.text, self.body_font, self.heading_font
        bs = int(self.body_size * 2)

        def style(sid, name, ppr="", rpr="", based="Normal", nxt="Normal", kind="paragraph", extra=""):
            b = f'<w:basedOn w:val="{based}"/>' if based else ""
            return (f'<w:style w:type="{kind}" w:styleId="{sid}"><w:name w:val="{name}"/>{b}'
                    f'<w:next w:val="{nxt}"/><w:qFormat/>{extra}<w:pPr>{ppr}</w:pPr><w:rPr>{rpr}</w:rPr></w:style>')

        hfont = f'<w:rFonts w:ascii="{hf}" w:hAnsi="{hf}" w:cs="{hf}"/>'
        return (
            XML + '<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
            f'<w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="{bf}" w:hAnsi="{bf}" w:eastAsia="{bf}" w:cs="{bf}"/>'
            f'<w:color w:val="{t}"/><w:sz w:val="{bs}"/><w:szCs w:val="{bs}"/><w:lang w:val="en-US"/></w:rPr></w:rPrDefault>'
            '<w:pPrDefault><w:pPr><w:spacing w:after="140" w:line="288" w:lineRule="auto"/></w:pPr></w:pPrDefault></w:docDefaults>'
            + style("Normal", "Normal", based=None)
            + style("Title", "Title", '<w:spacing w:before="0" w:after="120" w:line="240" w:lineRule="auto"/>',
                    f'{hfont}<w:b/><w:color w:val="{a}"/><w:sz w:val="64"/><w:szCs w:val="64"/>')
            + style("Subtitle", "Subtitle", '<w:spacing w:after="360"/>',
                    f'<w:color w:val="666666"/><w:sz w:val="{bs + 8}"/><w:szCs w:val="{bs + 8}"/>')
            + style("Heading1", "heading 1", '<w:keepNext/><w:spacing w:before="360" w:after="120"/><w:outlineLvl w:val="0"/>',
                    f'{hfont}<w:b/><w:color w:val="{a}"/><w:sz w:val="34"/><w:szCs w:val="34"/>')
            + style("Heading2", "heading 2", '<w:keepNext/><w:spacing w:before="240" w:after="80"/><w:outlineLvl w:val="1"/>',
                    f'{hfont}<w:b/><w:color w:val="{t}"/><w:sz w:val="26"/><w:szCs w:val="26"/>')
            + style("Heading3", "heading 3", '<w:keepNext/><w:spacing w:before="200" w:after="60"/><w:outlineLvl w:val="2"/>',
                    f'{hfont}<w:b/><w:caps/><w:color w:val="{a}"/><w:spacing w:val="20"/><w:sz w:val="20"/><w:szCs w:val="20"/>')
            + style("ListParagraph", "List Paragraph", '<w:ind w:left="720"/>')
            + "</w:styles>"
        )

    def _numbering(self):
        def abstract(aid, fmt, text, font=None):
            rpr = f'<w:rPr><w:rFonts w:ascii="{font}" w:hAnsi="{font}"/><w:color w:val="{self.accent}"/></w:rPr>' if font else \
                f'<w:rPr><w:color w:val="{self.accent}"/></w:rPr>'
            return (f'<w:abstractNum w:abstractNumId="{aid}"><w:multiLevelType w:val="singleLevel"/>'
                    f'<w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/><w:lvlText w:val="{text}"/>'
                    '<w:lvlJc w:val="left"/><w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr>'
                    f"{rpr}</w:lvl></w:abstractNum>")

        return (
            XML + '<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
            + abstract(0, "bullet", "•", self.body_font)
            + abstract(1, "decimal", "%1.")
            + '<w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>'
            + '<w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num></w:numbering>'
        )

    def save(self, path):
        w, h = (11906, 16838) if self.page == "A4" else (12240, 15840)
        m = self.margins
        doc = (
            XML + '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" '
            f'xmlns:r="{REL}"><w:body>' + "".join(self.body)
            + f'<w:sectPr><w:pgSz w:w="{w}" w:h="{h}"/>'
            f'<w:pgMar w:top="{m}" w:right="{m}" w:bottom="{m}" w:left="{m}" w:header="708" w:footer="708" w:gutter="0"/>'
            "</w:sectPr></w:body></w:document>"
        )
        settings = (XML + '<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                    '<w:defaultTabStop w:val="720"/><w:compat><w:compatSetting w:name="compatibilityMode" '
                    'w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>')
        ct = (
            XML + '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
            '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
            '<Default Extension="xml" ContentType="application/xml"/>'
            '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
            '<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>'
            '<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>'
            '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>'
            '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
            '<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>'
            "</Types>"
        )
        write_zip(path, {
            "[Content_Types].xml": ct,
            "_rels/.rels": rels([("rId1", "officeDocument", "word/document.xml"), ("rId2", CORE_REL, "docProps/core.xml"),
                                 ("rId3", APP_REL, "docProps/app.xml")]),
            "word/_rels/document.xml.rels": rels([("rId1", "styles", "styles.xml"), ("rId2", "numbering", "numbering.xml"),
                                                  ("rId3", "settings", "settings.xml")]),
            "word/document.xml": doc,
            "word/styles.xml": self._styles(),
            "word/numbering.xml": self._numbering(),
            "word/settings.xml": settings,
            "docProps/core.xml": core_props(self.title),
            "docProps/app.xml": app_props("Craft Suite"),
        })


# ---------------------------------------------------------------- Excel


def col_name(i):
    """0 → A, 25 → Z, 26 → AA."""
    s = ""
    i += 1
    while i:
        i, r = divmod(i - 1, 26)
        s = chr(65 + r) + s
    return s


def ref(col, row):
    return f"{col_name(col)}{row + 1}"


class Style:
    """A cell format: font, fill, border, number format, alignment (all optional)."""

    def __init__(self, bold=False, italic=False, size=11, color="222222", font="Calibri", fill=None, border=None,
                 num=None, h=None, v="center", wrap=False, indent=0):
        self.key = (bold, italic, size, color, font, fill, border, num, h, v, wrap, indent)


class Xlsx:
    def __init__(self, title, font="Calibri"):
        self.title = title
        self.font = font
        self.sheets = []
        self.strings, self.string_ix = [], {}
        self.fonts, self.fills, self.borders, self.numfmts, self.xfs = [], [], [], {}, []
        self._font(Style(font=font).key)
        self.fills = ['<fill><patternFill patternType="none"/></fill>', '<fill><patternFill patternType="gray125"/></fill>']
        self.borders = ["<border><left/><right/><top/><bottom/><diagonal/></border>"]
        self.xfs = ['<xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>']
        self.xf_ix = {}

    def _font(self, key):
        bold, italic, size, color, font = key[:5]
        x = (f"<font>{'<b/>' if bold else ''}{'<i/>' if italic else ''}<sz val=\"{size}\"/>"
             f'<color rgb="FF{color}"/><name val="{font}"/><family val="2"/></font>')
        if x not in self.fonts:
            self.fonts.append(x)
        return self.fonts.index(x)

    def style(self, s):
        if s is None:
            return 0
        if s.key in self.xf_ix:
            return self.xf_ix[s.key]
        bold, italic, size, color, font, fill, border, num, h, v, wrap, indent = s.key
        fid = self._font(s.key)
        fill_id = 0
        if fill:
            x = f'<fill><patternFill patternType="solid"><fgColor rgb="FF{fill}"/><bgColor indexed="64"/></patternFill></fill>'
            if x not in self.fills:
                self.fills.append(x)
            fill_id = self.fills.index(x)
        border_id = 0
        if border:
            # border: (colour, sides) — sides a string of "lrtb", or "all".
            color, sides = border if isinstance(border, tuple) else (border, "all")
            sides = "lrtb" if sides == "all" else sides

            def side(tag, letter):
                return f'<{tag} style="thin"><color rgb="FF{color}"/></{tag}>' if letter in sides else f"<{tag}/>"

            x = f"<border>{side('left', 'l')}{side('right', 'r')}{side('top', 't')}{side('bottom', 'b')}<diagonal/></border>"
            if x not in self.borders:
                self.borders.append(x)
            border_id = self.borders.index(x)
        num_id = 0
        if num:
            builtin = {"0": 1, "0.00": 2, "#,##0": 3, "#,##0.00": 4, "0%": 9, "0.00%": 10}
            if num in builtin:
                num_id = builtin[num]
            else:
                num_id = self.numfmts.setdefault(num, 164 + len(self.numfmts))
        al = ""
        if h or v or wrap or indent:
            al = ("<alignment" + (f' horizontal="{h}"' if h else "") + (f' vertical="{v}"' if v else "")
                  + (' wrapText="1"' if wrap else "") + (f' indent="{indent}"' if indent else "") + "/>")
        x = (f'<xf numFmtId="{num_id}" fontId="{fid}" fillId="{fill_id}" borderId="{border_id}" xfId="0"'
             f'{" applyNumberFormat=\"1\"" if num_id else ""} applyFont="1"{" applyFill=\"1\"" if fill_id else ""}'
             f'{" applyBorder=\"1\"" if border_id else ""}{" applyAlignment=\"1\"" if al else ""}>{al}</xf>')
        self.xfs.append(x)
        self.xf_ix[s.key] = len(self.xfs) - 1
        return self.xf_ix[s.key]

    def sheet(self, name, tab=None):
        sh = Sheet(self, name, tab)
        self.sheets.append(sh)
        return sh

    def _string(self, text):
        if text not in self.string_ix:
            self.string_ix[text] = len(self.strings)
            self.strings.append(text)
        return self.string_ix[text]

    def save(self, path):
        sheets_xml = "".join(f'<sheet name="{esc(s.name)}" sheetId="{i + 1}" r:id="rId{i + 1}"/>' for i, s in enumerate(self.sheets))
        n = len(self.sheets)
        workbook = (XML + '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
                    f'xmlns:r="{REL}"><bookViews><workbookView activeTab="0"/></bookViews><sheets>{sheets_xml}</sheets>'
                    '<calcPr calcId="191029" fullCalcOnLoad="1"/></workbook>')
        wb_rels = [(f"rId{i + 1}", "worksheet", f"worksheets/sheet{i + 1}.xml") for i in range(n)]
        wb_rels += [(f"rId{n + 1}", "styles", "styles.xml"), (f"rId{n + 2}", "sharedStrings", "sharedStrings.xml")]
        parts = {}
        for i, s in enumerate(self.sheets):
            parts[f"xl/worksheets/sheet{i + 1}.xml"] = s.xml(i == 0)
        numfmts = "".join(f'<numFmt numFmtId="{i}" formatCode="{esc(c)}"/>' for c, i in self.numfmts.items())
        styles = (XML + '<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">'
                  + (f'<numFmts count="{len(self.numfmts)}">{numfmts}</numFmts>' if self.numfmts else "")
                  + f'<fonts count="{len(self.fonts)}">{"".join(self.fonts)}</fonts>'
                  + f'<fills count="{len(self.fills)}">{"".join(self.fills)}</fills>'
                  + f'<borders count="{len(self.borders)}">{"".join(self.borders)}</borders>'
                  + '<cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>'
                  + f'<cellXfs count="{len(self.xfs)}">{"".join(self.xfs)}</cellXfs>'
                  + '<cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>')
        sst = (XML + f'<sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" count="{len(self.strings)}" '
               f'uniqueCount="{len(self.strings)}">' + "".join(f'<si><t xml:space="preserve">{esc(t)}</t></si>' for t in self.strings)
               + "</sst>")
        ct = (XML + '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
              '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
              '<Default Extension="xml" ContentType="application/xml"/>'
              '<Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>'
              + "".join(f'<Override PartName="/xl/worksheets/sheet{i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>' for i in range(n))
              + '<Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>'
              '<Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/>'
              '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
              '<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>'
              "</Types>")
        parts.update({
            "[Content_Types].xml": ct,
            "_rels/.rels": rels([("rId1", "officeDocument", "xl/workbook.xml"), ("rId2", CORE_REL, "docProps/core.xml"),
                                 ("rId3", APP_REL, "docProps/app.xml")]),
            "xl/workbook.xml": workbook,
            "xl/_rels/workbook.xml.rels": rels(wb_rels),
            "xl/styles.xml": styles,
            "xl/sharedStrings.xml": sst,
            "docProps/core.xml": core_props(self.title),
            "docProps/app.xml": app_props("Craft Suite"),
        })
        write_zip(path, parts)


class Sheet:
    def __init__(self, book, name, tab):
        self.book, self.name, self.tab = book, name, tab
        self.cells = {}  # (row, col) → (kind, value, xf)
        self.widths, self.heights, self.merges = {}, {}, []
        self.freeze = None
        self.gridlines = True

    def set(self, row, col, value, style=None):
        """A number, text, or a formula given as "=…"."""
        xf = self.book.style(style)
        if isinstance(value, str) and value.startswith("="):
            self.cells[(row, col)] = ("f", value[1:], xf)
        elif isinstance(value, (int, float)):
            self.cells[(row, col)] = ("n", value, xf)
        elif value is None:
            self.cells[(row, col)] = ("e", None, xf)
        else:
            self.cells[(row, col)] = ("s", self.book._string(str(value)), xf)

    def row(self, row, col, values, style=None, styles=None):
        for i, v in enumerate(values):
            self.set(row, col + i, v, styles[i] if styles else style)

    def fill(self, r0, c0, r1, c1, style):
        """Style a block (keeping values)."""
        xf = self.book.style(style)
        for r in range(r0, r1 + 1):
            for c in range(c0, c1 + 1):
                kind, v, _ = self.cells.get((r, c), ("e", None, 0))
                self.cells[(r, c)] = (kind, v, xf)

    def merge(self, r0, c0, r1, c1):
        self.merges.append(f"{ref(c0, r0)}:{ref(c1, r1)}")

    def xml(self, selected):
        rows = {}
        for (r, c), cell in self.cells.items():
            rows.setdefault(r, {})[c] = cell
        out = [XML, '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
                    f'xmlns:r="{REL}">']
        if self.tab:
            out.append(f'<sheetPr><tabColor rgb="FF{self.tab}"/></sheetPr>')
        sv = f'<sheetView{" tabSelected=\"1\"" if selected else ""}{"" if self.gridlines else " showGridLines=\"0\""} workbookViewId="0">'
        if self.freeze:
            fr, fc = self.freeze
            attrs = (f' xSplit="{fc}"' if fc else "") + (f' ySplit="{fr}"' if fr else "")
            pane = "bottomRight" if fr and fc else ("bottomLeft" if fr else "topRight")
            sv += f'<pane{attrs} topLeftCell="{ref(fc, fr)}" activePane="{pane}" state="frozen"/>'
        out.append(f"<sheetViews>{sv}</sheetView></sheetViews>")
        out.append('<sheetFormatPr defaultRowHeight="18"/>')
        if self.widths:
            out.append("<cols>" + "".join(f'<col min="{c + 1}" max="{c + 1}" width="{w}" customWidth="1"/>'
                                          for c, w in sorted(self.widths.items())) + "</cols>")
        out.append("<sheetData>")
        for r in sorted(set(rows) | set(self.heights)):
            ht = f' ht="{self.heights[r]}" customHeight="1"' if r in self.heights else ""
            out.append(f'<row r="{r + 1}"{ht}>')
            for c in sorted(rows.get(r, {})):
                kind, v, xf = rows[r][c]
                s = f' s="{xf}"' if xf else ""
                if kind == "f":
                    out.append(f'<c r="{ref(c, r)}"{s}><f>{esc(v)}</f></c>')
                elif kind == "n":
                    out.append(f'<c r="{ref(c, r)}"{s}><v>{v}</v></c>')
                elif kind == "s":
                    out.append(f'<c r="{ref(c, r)}"{s} t="s"><v>{v}</v></c>')
                else:
                    out.append(f'<c r="{ref(c, r)}"{s}/>')
            out.append("</row>")
        out.append("</sheetData>")
        if self.merges:
            out.append(f'<mergeCells count="{len(self.merges)}">' + "".join(f'<mergeCell ref="{m}"/>' for m in self.merges) + "</mergeCells>")
        out.append('<pageMargins left="0.5" right="0.5" top="0.6" bottom="0.6" header="0.3" footer="0.3"/>')
        out.append('<pageSetup orientation="portrait" fitToWidth="1" fitToHeight="0"/>')
        out.append("</worksheet>")
        return "".join(out)


# ---------------------------------------------------------------- PowerPoint

EMU = 12700  # per point
SLIDE_W, SLIDE_H = 12192000, 6858000  # 16:9, 13.333 × 7.5 in


def emu(inches):
    return int(inches * 914400)


A = "http://schemas.openxmlformats.org/drawingml/2006/main"
P = "http://schemas.openxmlformats.org/presentationml/2006/main"


class Pptx:
    """Slides of plain shapes and text boxes, in a theme of two colours and two fonts."""

    def __init__(self, title, colors, heading_font="Montserrat", body_font="Open Sans"):
        """colors: dk1, lt1, dk2, lt2, accent1…accent6 as hex."""
        self.title = title
        self.colors = colors
        self.heading_font, self.body_font = heading_font, body_font
        self.slides = []

    def slide(self, background):
        s = Slide(self, background)
        self.slides.append(s)
        return s

    def _theme(self):
        c = self.colors
        names = ["dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3", "accent4", "accent5", "accent6", "hlink", "folHlink"]
        scheme = "".join(f'<a:{n}><a:srgbClr val="{c[n]}"/></a:{n}>' for n in names)
        fill = '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
        line = '<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
        return (XML + f'<a:theme xmlns:a="{A}" name="{esc(self.title)}"><a:themeElements>'
                f'<a:clrScheme name="{esc(self.title)}">{scheme}</a:clrScheme>'
                f'<a:fontScheme name="{esc(self.title)}"><a:majorFont><a:latin typeface="{self.heading_font}"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>'
                f'<a:minorFont><a:latin typeface="{self.body_font}"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>'
                '<a:fmtScheme name="Craft"><a:fillStyleLst>' + fill * 3 + "</a:fillStyleLst>"
                "<a:lnStyleLst>" + line * 3 + "</a:lnStyleLst>"
                "<a:effectStyleLst>" + "<a:effectStyle><a:effectLst/></a:effectStyle>" * 3 + "</a:effectStyleLst>"
                "<a:bgFillStyleLst>" + fill * 3 + "</a:bgFillStyleLst></a:fmtScheme>"
                "</a:themeElements></a:theme>")

    def _master(self):
        return (XML + f'<p:sldMaster xmlns:a="{A}" xmlns:r="{REL}" xmlns:p="{P}"><p:cSld><p:bg><p:bgRef idx="1001"><a:schemeClr val="bg1"/></p:bgRef></p:bg>'
                '<p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/></p:spTree></p:cSld>'
                '<p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" '
                'accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/>'
                '<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst>'
                "<p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>")

    def _layout(self):
        return (XML + f'<p:sldLayout xmlns:a="{A}" xmlns:r="{REL}" xmlns:p="{P}" type="blank" preserve="1">'
                '<p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
                "<p:grpSpPr/></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>")

    def save(self, path):
        n = len(self.slides)
        pres = (XML + f'<p:presentation xmlns:a="{A}" xmlns:r="{REL}" xmlns:p="{P}" saveSubsetFonts="1">'
                '<p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId1"/></p:sldMasterIdLst>'
                "<p:sldIdLst>" + "".join(f'<p:sldId id="{256 + i}" r:id="rId{i + 3}"/>' for i in range(n)) + "</p:sldIdLst>"
                f'<p:sldSz cx="{SLIDE_W}" cy="{SLIDE_H}"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>')
        pres_rels = [("rId1", "slideMaster", "slideMasters/slideMaster1.xml"), ("rId2", "theme", "theme/theme1.xml")]
        pres_rels += [(f"rId{i + 3}", "slide", f"slides/slide{i + 1}.xml") for i in range(n)]
        parts = {
            "ppt/presentation.xml": pres,
            "ppt/_rels/presentation.xml.rels": rels(pres_rels),
            "ppt/theme/theme1.xml": self._theme(),
            "ppt/slideMasters/slideMaster1.xml": self._master(),
            "ppt/slideMasters/_rels/slideMaster1.xml.rels": rels([("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml"),
                                                                  ("rId2", "theme", "../theme/theme1.xml")]),
            "ppt/slideLayouts/slideLayout1.xml": self._layout(),
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels": rels([("rId1", "slideMaster", "../slideMasters/slideMaster1.xml")]),
            "docProps/core.xml": core_props(self.title),
            "docProps/app.xml": app_props("Craft Suite"),
        }
        for i, s in enumerate(self.slides):
            parts[f"ppt/slides/slide{i + 1}.xml"] = s.xml()
            parts[f"ppt/slides/_rels/slide{i + 1}.xml.rels"] = rels([("rId1", "slideLayout", "../slideLayouts/slideLayout1.xml")])
        ct = (XML + '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
              '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
              '<Default Extension="xml" ContentType="application/xml"/>'
              '<Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>'
              '<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>'
              '<Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/>'
              '<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>'
              + "".join(f'<Override PartName="/ppt/slides/slide{i + 1}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>' for i in range(n))
              + '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
              '<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>'
              "</Types>")
        parts["[Content_Types].xml"] = ct
        parts["_rels/.rels"] = rels([("rId1", "officeDocument", "ppt/presentation.xml"), ("rId2", CORE_REL, "docProps/core.xml"),
                                     ("rId3", APP_REL, "docProps/app.xml")])
        write_zip(path, parts)


class Slide:
    def __init__(self, deck, background):
        self.deck, self.background = deck, background
        self.shapes = []

    def _id(self):
        return len(self.shapes) + 2

    def shape(self, x, y, w, h, fill=None, geom="rect", line=None, radius=None, alpha=None):
        """A filled shape at (x, y) inches, w × h inches."""
        fill_xml = "<a:noFill/>"
        if fill:
            a = f'<a:alpha val="{int(alpha * 1000)}"/>' if alpha is not None else ""
            fill_xml = f'<a:solidFill><a:srgbClr val="{fill}">{a}</a:srgbClr></a:solidFill>'
        ln = f'<a:ln w="{int(line[1] * EMU)}"><a:solidFill><a:srgbClr val="{line[0]}"/></a:solidFill></a:ln>' if line else "<a:ln><a:noFill/></a:ln>"
        av = f'<a:avLst><a:gd name="adj" fmla="val {radius}"/></a:avLst>' if radius is not None else "<a:avLst/>"
        i = self._id()
        self.shapes.append(
            f'<p:sp><p:nvSpPr><p:cNvPr id="{i}" name="Shape {i}"/><p:cNvSpPr/><p:nvPr/></p:nvSpPr>'
            f'<p:spPr><a:xfrm><a:off x="{emu(x)}" y="{emu(y)}"/><a:ext cx="{emu(w)}" cy="{emu(h)}"/></a:xfrm>'
            f'<a:prstGeom prst="{geom}">{av}</a:prstGeom>{fill_xml}{ln}</p:spPr>'
            '<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:endParaRPr lang="en-US"/></a:p></p:txBody></p:sp>')

    def text(self, x, y, w, h, paras, anchor="t", fill=None, inset=0.05, autofit=False):
        """paras: list of (runs, {align, size, color, bold, font, space_after, bullet, line}); runs: str or
        [(text, {size, color, bold, italic, font})]."""
        body = []
        for para in paras:
            if isinstance(para, str):
                para = (para, {})
            runs, pf = para
            if isinstance(runs, str):
                runs = [(runs, {})]
            ppr = []
            algn = {"l": "l", "c": "ctr", "r": "r"}.get(pf.get("align", "l"), "l")
            attrs = f' algn="{algn}"'
            if pf.get("bullet"):
                attrs += ' marL="285750" indent="-285750"'
            inner = ""
            if pf.get("line"):
                inner += f'<a:lnSpc><a:spcPct val="{int(pf["line"] * 100000)}"/></a:lnSpc>'
            if pf.get("space_after") is not None:
                inner += f'<a:spcAft><a:spcPts val="{int(pf["space_after"] * 100)}"/></a:spcAft>'
            if pf.get("bullet"):
                inner += (f'<a:buClr><a:srgbClr val="{pf.get("bullet_color", pf.get("color", "333333"))}"/></a:buClr>'
                          '<a:buFont typeface="Arial"/><a:buChar char="•"/>')
            else:
                inner += "<a:buNone/>"
            ppr.append(f"<a:pPr{attrs}>{inner}</a:pPr>")
            rs = []
            for text, rf in runs:
                f = {**pf, **rf}
                size = int(f.get("size", 18) * 100)
                b = ' b="1"' if f.get("bold") else ""
                it = ' i="1"' if f.get("italic") else ""
                sp = f' spc="{int(f["spacing"] * 100)}"' if f.get("spacing") else ""
                font = f.get("font", self.deck.body_font)
                rs.append(f'<a:r><a:rPr lang="en-US" sz="{size}"{b}{it}{sp} dirty="0">'
                          f'<a:solidFill><a:srgbClr val="{f.get("color", "222222")}"/></a:solidFill>'
                          f'<a:latin typeface="{font}"/></a:rPr><a:t>{esc(text)}</a:t></a:r>')
            body.append(f"<a:p>{''.join(ppr)}{''.join(rs)}</a:p>")
        i = self._id()
        fill_xml = f'<a:solidFill><a:srgbClr val="{fill}"/></a:solidFill>' if fill else "<a:noFill/>"
        ins = emu(inset)
        fit = "<a:normAutofit/>" if autofit else ""
        self.shapes.append(
            f'<p:sp><p:nvSpPr><p:cNvPr id="{i}" name="Text {i}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>'
            f'<p:spPr><a:xfrm><a:off x="{emu(x)}" y="{emu(y)}"/><a:ext cx="{emu(w)}" cy="{emu(h)}"/></a:xfrm>'
            f'<a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}</p:spPr>'
            f'<p:txBody><a:bodyPr wrap="square" lIns="{ins}" tIns="{ins}" rIns="{ins}" bIns="{ins}" anchor="{anchor}">{fit}</a:bodyPr>'
            f"<a:lstStyle/>{''.join(body)}</p:txBody></p:sp>")

    def xml(self):
        bg = f'<p:bg><p:bgPr><a:solidFill><a:srgbClr val="{self.background}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>'
        return (XML + f'<p:sld xmlns:a="{A}" xmlns:r="{REL}" xmlns:p="{P}"><p:cSld>{bg}<p:spTree>'
                '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>'
                '<p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>'
                + "".join(self.shapes) + "</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>")
