// HTML / PDF / DOCX export of the current document, styled with the active
// Word theme and laid out on the chosen page size.

import { MarkdownToHtml } from '@muyajs/core'
import { currentMarkdown, docAssetUrl, getMuya } from './editor'
import { baseName, doc, saveExport } from './document'
import { blobToDataUrl, callNative, isNative } from './native'
import { settings } from './settings'
import { getPageDimensions, MM_TO_PX, themeCss } from './theme'
import { prepareDocxHtml } from '../../desktop/src/renderer/src/util/docxHtml'
import { toast } from './ui'

const EXPORT_CSS = `
html, body { background: var(--editorBgColor); margin: 0; }
.markdown-body { color: var(--editorColor); background: var(--editorBgColor); }
.markdown-body h1 { color: var(--h1Color); }
.markdown-body h2 { color: var(--h2Color); }
.markdown-body h3 { color: var(--h3Color); }
.markdown-body h4 { color: var(--h4Color); }
.markdown-body h5 { color: var(--h5Color); }
.markdown-body h6 { color: var(--h6Color); }
.markdown-body a { color: var(--linkColor); }
.markdown-body blockquote { color: var(--blockquoteTextColor); border-left-color: var(--blockquoteBorderColor); }
.markdown-body code { background: var(--codeBgColor); }
.markdown-body pre { background: var(--codeBlockBgColor); color: var(--codeBlockColor); }
.markdown-body table th, .markdown-body table td { border-color: var(--tableBorderColor); }
.markdown-body table tr { background: transparent; border-color: var(--tableBorderColor); }
.markdown-body hr { background: var(--hrColor); }
.markdown-body img { max-width: 100%; }
@media print {
  html, body { -webkit-print-color-adjust: exact; print-color-adjust: exact; }
  .markdown-body { max-width: none; padding: 0; }
}`

const pageCss = (): string => {
  const page = getPageDimensions(settings.pageLayout) ?? getPageDimensions('a4')!
  return `@page { size: ${page.width}mm ${page.height}mm; margin: ${settings.pageMargin}mm; }`
}

const IMG_SRC_REG = /(<img\b[^>]*?\ssrc=")([^"]+)(")/gi
const ABSOLUTE_SRC_REG = /^(?:[a-z][a-z\d+.-]+:|\/\/|#)/i
const DOC_FILE_PREFIX = 'file:///__markd_doc/'
const DOC_ROUTE_REG = new RegExp(`^${location.origin.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}/doc/[^/]+/(.+)$`)

/** Document-relative path of an exported image src, or null for absolute ones. */
const relativeSrc = (raw: string): string | null => {
  const src = raw.replace(/&amp;/g, '&')
  if (src.startsWith(DOC_FILE_PREFIX)) return src.slice(DOC_FILE_PREFIX.length)
  // Already rewritten by the editor's image shim while the export rendered.
  const served = DOC_ROUTE_REG.exec(src)
  if (served) return served[1]!
  return ABSOLUTE_SRC_REG.test(src) ? null : src
}

/** Exports must be self-contained: embed document-relative images as data URIs. */
const inlineRelativeImages = async(html: string): Promise<string> => {
  const sources = new Set<string>()
  for (const match of html.matchAll(IMG_SRC_REG)) {
    const src = relativeSrc(match[2]!)
    if (src) sources.add(src)
  }
  const replacements = new Map<string, string>()
  await Promise.all([...sources].map(async(src) => {
    try {
      const response = await fetch(docAssetUrl(src))
      if (response.ok) replacements.set(src, await blobToDataUrl(await response.blob()))
    } catch {
      // Leave the relative path; the image just will not show in the export.
    }
  }))
  return html.replace(IMG_SRC_REG, (match, pre: string, src: string, post: string) => {
    const relative = relativeSrc(src)
    const dataUrl = relative ? replacements.get(relative) : undefined
    return dataUrl ? `${pre}${dataUrl}${post}` : match
  })
}

const buildHtml = async(forPrint: boolean): Promise<string> => {
  const muya = getMuya()
  const html = await new MarkdownToHtml(currentMarkdown(), muya).generate({
    title: baseName(),
    extraCSS: `${themeCss()}\n${EXPORT_CSS}\n${forPrint ? pageCss() : ''}`
  })
  return inlineRelativeImages(html)
}

export const exportHtml = async(): Promise<void> => {
  toast('Preparing HTML…')
  const html = await buildHtml(false)
  await saveExport(new Blob([html], { type: 'text/html' }), `${baseName()}.html`)
}

/** Android prints through the system print dialog, whose "Save as PDF" target writes the file. */
export const exportPdf = async(): Promise<void> => {
  toast('Preparing PDF…')
  const html = await buildHtml(true)
  const page = getPageDimensions(settings.pageLayout) ?? getPageDimensions('a4')!
  if (isNative()) {
    await callNative('printHtml', { html, jobName: baseName(), pageSize: page.printName })
    return
  }
  const frame = document.createElement('iframe')
  frame.style.cssText = 'position:fixed;width:0;height:0;border:0;'
  frame.srcdoc = html
  frame.onload = () => {
    frame.contentWindow?.print()
    setTimeout(() => frame.remove(), 60_000)
  }
  document.body.append(frame)
}

const MM_TO_TWIP = 1440 / 25.4
const toTwip = (mm: number): number => Math.round(mm * MM_TO_TWIP)

/** Paint the Word page colour (w:background) and tell Word to display it. */
const applyPageBackground = async(docx: Blob, color: string): Promise<Blob> => {
  const { default: JSZip } = await import('jszip')
  const zip = await JSZip.loadAsync(docx)
  const documentFile = zip.file('word/document.xml')
  const settingsFile = zip.file('word/settings.xml')
  if (!documentFile || !settingsFile) return docx
  let documentXml = await documentFile.async('string')
  if (!documentXml.includes('<w:background')) {
    documentXml = documentXml.replace(/(<w:document\b[^>]*>)/, `$1<w:background w:color="${color}"/>`)
  }
  let settingsXml = await settingsFile.async('string')
  if (!settingsXml.includes('w:displayBackgroundShape')) {
    // CT_Settings order: w:zoom precedes w:displayBackgroundShape.
    settingsXml = /<w:zoom\b[^>]*\/>/.test(settingsXml)
      ? settingsXml.replace(/(<w:zoom\b[^>]*\/>)/, '$1<w:displayBackgroundShape/>')
      : settingsXml.replace(/(<w:settings\b[^>]*>)/, '$1<w:displayBackgroundShape/>')
  }
  zip.file('word/document.xml', documentXml)
  zip.file('word/settings.xml', settingsXml)
  return zip.generateAsync({ type: 'blob', compression: 'DEFLATE', mimeType: DOCX_MIME })
}

const DOCX_MIME = 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'

export const exportDocx = async(): Promise<void> => {
  toast('Preparing Word document…')
  try {
    const page = getPageDimensions(settings.pageLayout) ?? getPageDimensions('a4')!
    const margin = settings.pageMargin
    const html = await buildHtml(false)
    const contentWidthPx = (page.width - 2 * margin) * MM_TO_PX
    const prepared = await prepareDocxHtml(html, contentWidthPx)
    // html-to-docx decodes embedded images with Node's Buffer, even in its browser build.
    const globals = globalThis as { Buffer?: unknown }
    globals.Buffer ??= (await import('buffer')).Buffer
    const { default: HTMLtoDOCX } = await import('@turbodocx/html-to-docx')
    const result = await HTMLtoDOCX(prepared.html, null, {
      orientation: 'portrait',
      pageSize: { width: toTwip(page.width), height: toTwip(page.height) },
      margins: {
        top: toTwip(margin),
        right: toTwip(margin),
        bottom: toTwip(margin),
        left: toTwip(margin),
        header: toTwip(Math.min(12.7, margin / 2)),
        footer: toTwip(Math.min(12.7, margin / 2)),
        gutter: 0
      },
      title: baseName(),
      creator: 'Markd',
      font: 'Calibri',
      // html-to-docx takes half-points; 1px = 0.75pt.
      fontSize: Math.round(settings.fontSize * 0.75 * 2),
      table: { row: { cantSplit: true } },
      lineNumber: false
    })
    let blob = result instanceof Blob ? result : new Blob([result as ArrayBuffer], { type: DOCX_MIME })
    const pageColor = prepared.backgroundColor.replace('#', '').toUpperCase()
    if (/^[0-9A-F]{6}$/.test(pageColor) && pageColor !== 'FFFFFF') {
      blob = await applyPageBackground(blob, pageColor)
    }
    await saveExport(new Blob([blob], { type: DOCX_MIME }), `${baseName()}.docx`)
  } catch (err) {
    toast(`DOCX export failed: ${(err as Error).message}`)
  }
}

export const shareDocument = async(): Promise<void> => {
  if (!doc.uri) {
    toast('Save the document first')
    return
  }
  await callNative('share', { uri: doc.uri, mime: 'text/markdown' }).catch((err: Error) => toast(err.message))
}
