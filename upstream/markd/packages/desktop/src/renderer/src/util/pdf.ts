// `escapeHTML`/`unescapeHTML` are migrated to @muyajs/core (identical impl).
// The TOC anchors produced here (`#${slug}`) must match the heading `id`
// attributes in the exported document. Now that editor.vue exports via
// @muyajs/core (#4406) and the engine injects github-compatible heading ids
// (#4412), this module derives its slugs from the SAME `generateGithubSlug`
// algorithm, with the SAME `-N` document-order dedup the engine uses, so the
// in-document TOC links resolve.
import { escapeHTML, unescapeHTML, generateGithubSlug } from '@muyajs/core'
import academicTheme from '@/assets/themes/export/academic.theme.css?inline'
import liberTheme from '@/assets/themes/export/liber.theme.css?inline'
import { deepClone } from '../util'
import { sanitize, EXPORT_DOMPURIFY_CONFIG } from '../util/dompurify'

export interface PdfCssOptions {
  type?: string
  pageMarginTop?: number
  pageMarginRight?: number
  pageMarginBottom?: number
  pageMarginLeft?: number
  fontFamily?: string
  fontSize?: number
  lineHeight?: number | string
  autoNumberingHeadings?: boolean
  showFrontMatter?: boolean
  theme?: string
  /** Raw CSS of the active editor theme, used when `theme === 'editor'`. */
  editorThemeCss?: string
  /** Resolved editor background colour, used to paint the page margins. */
  editorBgColor?: string
  headerFooterFontSize?: number
  [key: string]: unknown
}

// Maps the editor theme variables onto the exported `.markdown-body`, so a
// PDF/HTML/DOCX export looks like the document does in the editor. Every
// variable has a fallback for themes (e.g. the default light theme) that only
// define part of the palette.
const EDITOR_THEME_EXPORT_CSS = `
html, body { background: var(--editorBgColor, #ffffff) !important; }
article.print-container, .markdown-body {
  background: var(--editorBgColor, #ffffff) !important;
  color: var(--editorColor, #24292f) !important;
  -webkit-print-color-adjust: exact;
  print-color-adjust: exact;
}
.markdown-body h1 { color: var(--h1Color, var(--headingColor, inherit)); }
.markdown-body h2 { color: var(--h2Color, var(--headingColor, inherit)); }
.markdown-body h3 { color: var(--h3Color, var(--headingColor, inherit)); }
.markdown-body h4 { color: var(--h4Color, var(--headingColor, inherit)); }
.markdown-body h5 { color: var(--h5Color, var(--headingColor, inherit)); }
.markdown-body h6 { color: var(--h6Color, var(--headingColor, inherit)); }
.markdown-body h1, .markdown-body h2 { border-bottom-color: var(--hrColor, var(--editorColor10, #d0d7de)); }
.markdown-body a { color: var(--linkColor, #0969da); }
.markdown-body strong { color: var(--strongColor, inherit); }
.markdown-body em { color: var(--emColor, inherit); }
.markdown-body li::marker { color: var(--listMarkerColor, inherit); }
.markdown-body blockquote {
  color: var(--blockquoteTextColor, var(--editorColor60, #59636e));
  border-left-color: var(--blockquoteBorderColor, var(--themeColor, #d0d7de));
}
.markdown-body hr { background-color: var(--hrColor, var(--editorColor10, #d0d7de)); border-color: var(--hrColor, var(--editorColor10, #d0d7de)); }
.markdown-body code:not(pre code) { background: var(--codeBgColor, rgba(175, 184, 193, 0.2)); color: inherit; }
.markdown-body pre, .markdown-body pre[class*='language-'], .markdown-body .highlight pre {
  background: var(--codeBlockBgColor, #f6f8fa) !important;
  color: var(--codeBlockColor, inherit);
}
.markdown-body table tr { background: transparent; border-top-color: var(--tableBorderColor, #d0d7de); }
.markdown-body table tr:nth-child(2n) { background: var(--editorColor04, rgba(0, 0, 0, 0.03)); }
.markdown-body table th, .markdown-body table td { border-color: var(--tableBorderColor, #d0d7de); }
.markdown-body mark { background: var(--highlightColor, #fff8c5); color: inherit; }
.markdown-body .token.operator, .markdown-body .token.entity, .markdown-body .token.url { background: none !important; }
`

export const getCssForOptions = async(options: PdfCssOptions): Promise<string> => {
  const {
    type,
    pageMarginTop,
    pageMarginRight,
    pageMarginBottom,
    pageMarginLeft,
    fontFamily,
    fontSize,
    lineHeight,
    autoNumberingHeadings,
    showFrontMatter,
    theme,
    editorThemeCss,
    editorBgColor,
    headerFooterFontSize
  } = options
  // DOCX styles are resolved on screen (see docxHtml.ts), so they must not be
  // wrapped in `@media print`.
  const isPrintable = type !== 'styledHtml' && type !== 'docx'

  let output = ''
  if (isPrintable) {
    output += `@media print{@page{
      margin: ${pageMarginTop}mm ${pageMarginRight}mm ${pageMarginBottom}mm ${pageMarginLeft}mm;}`
    // Keep a heading with the content that follows it, so a page never breaks
    // immediately after a heading, and never split a multi-line heading (#3039).
    output += 'h1,h2,h3,h4,h5,h6{break-after:avoid;break-inside:avoid;}'
  }

  // Auto numbering headings via CSS
  if (autoNumberingHeadings) {
    output += autoNumberingHeadingsCss
  }

  // Hide front matter
  if (!showFrontMatter) {
    output += 'pre.front-matter{display:none!important;}'
  }

  if (theme) {
    if (theme === 'editor') {
      output += (editorThemeCss || '') + EDITOR_THEME_EXPORT_CSS
      if (isPrintable && editorBgColor && /^[#a-z0-9(),.\s%]+$/i.test(editorBgColor)) {
        // Paint the page margins too, otherwise a dark theme prints inside a
        // white frame.
        output += `@page{background:${editorBgColor};}`
      }
    } else if (theme === 'academic') {
      output += academicTheme
    } else if (theme === 'liber') {
      output += liberTheme
    } else {
      // Read theme from disk
      const { userDataPath } = window.marktext!.paths as { userDataPath: string }
      const themePath = window.path.join(userDataPath, 'themes/export', theme)
      if (await window.fileUtils.isFile(themePath)) {
        try {
          const buf = await window.fileUtils.readFile(themePath)
          const themeCSS =
            buf instanceof Uint8Array ? new TextDecoder('utf-8').decode(buf) : String(buf)
          output += themeCSS
        } catch (_) {
          // No-op
        }
      }
    }
  }

  // Font options. Emitted AFTER the theme CSS so the "Overwrite theme font"
  // settings win the cascade: a selected export theme also sets `.markdown-body`
  // font-size/line-height/font-family, and at equal specificity the later rule
  // wins — so the user's override must come last to actually override the theme.
  output += '.markdown-body{'
  if (fontFamily) {
    output += `font-family:${NOTATION_FONT_FAMILIES},"${fontFamily}",${FALLBACK_FONT_FAMILIES};`
    output = `.hf-container{font-family:${NOTATION_FONT_FAMILIES},"${fontFamily}",${FALLBACK_FONT_FAMILIES};}${output}`
  }
  if (fontSize) {
    output += `font-size:${fontSize}px;`
  }
  if (lineHeight) {
    output += `line-height:${lineHeight};`
  }
  output += '}'

  if (headerFooterFontSize) {
    output += `.page-header .hf-container,
    .page-footer-fake .hf-container,
    .page-footer .hf-container {
      font-size: ${headerFooterFontSize}px;
    }`
  }

  if (isPrintable) {
    // Close @page
    output += '}'
  }
  return unescapeHTML(sanitize(escapeHTML(output), EXPORT_DOMPURIFY_CONFIG))
}

export interface TocEntry {
  lvl: number
  content: string
  contentHtml?: string
  slug?: string
}

export interface HtmlTocOptions {
  tocIncludeTopHeading?: boolean
  tocTitle?: string
  [key: string]: unknown
}

// Replicate @muyajs/core's `MarkdownToHtml#_injectHeadingIds` slugging so the
// TOC `href="#slug"` anchors target the exact ids the engine writes onto the
// exported `<h1>..<h6>`: github-compatible base slug (falling back to
// `heading` when the text slugs to empty), deduplicated in document order with
// an incrementing `-N` suffix. Computed over the FULL heading list in order
// (before the render-time filtering below) to keep the dedup sequence aligned
// with the engine's whole-document pass.
const assignHeadingSlugs = (tocList: TocEntry[]): void => {
  const seen = new Set<string>()
  for (const entry of tocList) {
    const base = generateGithubSlug(entry.content) || 'heading'
    let slug = base
    let n = 1
    while (seen.has(slug)) {
      slug = `${base}-${n++}`
    }
    seen.add(slug)
    entry.slug = slug
  }
}

const generateHtmlToc = (
  tocList: TocEntry[],
  currentLevel: number,
  options: HtmlTocOptions
): string => {
  if (!tocList || tocList.length === 0) {
    return ''
  }

  const topLevel = tocList[0].lvl
  if (!options.tocIncludeTopHeading && topLevel <= 1) {
    tocList.shift()
    return generateHtmlToc(tocList, currentLevel, options)
  } else if (topLevel <= currentLevel) {
    return ''
  }

  const shifted = tocList.shift() as TocEntry
  const { content, contentHtml, lvl, slug } = shifted

  const label = contentHtml ?? content
  let html = `<li><span><a class="toc-h${lvl}" href="#${slug}">${label}</a><span class="dots"></span></span>`

  // Generate sub-items
  if (tocList.length !== 0 && tocList[0].lvl > lvl) {
    html += '<ul>' + generateHtmlToc(tocList, lvl, options) + '</ul>'
  }

  html += '</li>' + generateHtmlToc(tocList, currentLevel, options)
  return html
}

export const getHtmlToc = (toc: TocEntry[], options: HtmlTocOptions = {}): string => {
  const list = deepClone(toc)
  assignHeadingSlugs(list)
  const tocList = generateHtmlToc(list, 0, options)
  if (!tocList) {
    return ''
  }

  const title = options.tocTitle ? options.tocTitle : 'Table of Contents'
  const html = `<div class="toc-container"><p class="toc-title">${title}</p><ul class="toc-list">${tocList}</ul></div>`
  return sanitize(html, EXPORT_DOMPURIFY_CONFIG)
}

// Don't use "Noto Color Emoji" because it will result in PDF files with multiple MB and weird looking emojis.
const FALLBACK_FONT_FAMILIES =
  '"Open Sans","Segoe UI","Helvetica Neue",Helvetica,Arial,sans-serif,"Apple Color Emoji","Segoe UI Emoji"'
const NOTATION_FONT_FAMILIES = '"Noto Music","Noto Sans Math"'

const autoNumberingHeadingsCss = `body {counter-reset: h2}
h2 {counter-reset: h3}
h3 {counter-reset: h4}
h4 {counter-reset: h5}
h5 {counter-reset: h6}
h2:before {counter-increment: h2; content: counter(h2) ". "}
h3:before {counter-increment: h3; content: counter(h2) "." counter(h3) ". "}
h4:before {counter-increment: h4; content: counter(h2) "." counter(h3) "." counter(h4) ". "}
h5:before {counter-increment: h5; content: counter(h2) "." counter(h3) "." counter(h4) "." counter(h5) ". "}
h6:before {counter-increment: h6; content: counter(h2) "." counter(h3) "." counter(h4) "." counter(h5) "." counter(h6) ". "}
h2.nocount:before, h3.nocount:before, h4.nocount:before, h5.nocount:before, h6.nocount:before { content: ""; counter-increment: none }`
