// Native DOCX export (no pandoc needed). The renderer hands us themed HTML
// whose computed styles were already inlined; this module turns it into a
// Word document with the requested page size, margins, default font and page
// colour.

import fs from 'fs/promises'
import path from 'path'
import { fileURLToPath } from 'url'
import HTMLtoDOCX from '@turbodocx/html-to-docx'
import JSZip from 'jszip'
import log from 'electron-log'
import type { DocxExportOptions } from '@shared/types/files'

const MM_TO_TWIP = 1440 / 25.4

const IMAGE_MIME: Readonly<Record<string, string>> = Object.freeze({
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.bmp': 'image/bmp',
  '.webp': 'image/webp',
  '.svg': 'image/svg+xml'
})

const IMG_FILE_SRC_REG = /(<img\b[^>]*?\ssrc=")(file:\/\/[^"]+)(")/gi

/** html-to-docx cannot read `file://` URLs, so embed local images as data URIs. */
const inlineLocalImages = async(html: string): Promise<string> => {
  const sources = new Set<string>()
  for (const match of html.matchAll(IMG_FILE_SRC_REG)) {
    sources.add(match[2]!)
  }

  const replacements = new Map<string, string>()
  await Promise.all(
    [...sources].map(async(src) => {
      try {
        const filePath = fileURLToPath(src.replace(/&amp;/g, '&'))
        const mime = IMAGE_MIME[path.extname(filePath).toLowerCase()]
        if (!mime) return
        const data = await fs.readFile(filePath)
        replacements.set(src, `data:${mime};base64,${data.toString('base64')}`)
      } catch (err) {
        log.warn('DOCX export: cannot embed image', src, err)
      }
    })
  )

  return html.replace(IMG_FILE_SRC_REG, (match, pre: string, src: string, post: string) => {
    const dataUri = replacements.get(src)
    return dataUri ? `${pre}${dataUri}${post}` : match
  })
}

const toTwip = (mm: number): number => Math.round(mm * MM_TO_TWIP)

const normalizeHex = (color?: string): string | null => {
  if (!color) return null
  const match = /^#?([0-9a-f]{6})$/i.exec(color.trim())
  return match ? match[1]!.toUpperCase() : null
}

/** Paint the Word page colour (w:background) and tell Word to display it. */
const applyPageBackground = async(docx: Buffer, color: string): Promise<Buffer> => {
  const zip = await JSZip.loadAsync(docx)
  const documentFile = zip.file('word/document.xml')
  const settingsFile = zip.file('word/settings.xml')
  if (!documentFile || !settingsFile) return docx

  let documentXml = await documentFile.async('string')
  if (!documentXml.includes('<w:background')) {
    documentXml = documentXml.replace(
      /(<w:document\b[^>]*>)/,
      `$1<w:background w:color="${color}"/>`
    )
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
  return zip.generateAsync({ type: 'nodebuffer', compression: 'DEFLATE' })
}

export const htmlToDocx = async(html: string, options: DocxExportOptions): Promise<Buffer> => {
  const content = await inlineLocalImages(html)
  const {
    pageWidthMm,
    pageHeightMm,
    landscape,
    marginTopMm,
    marginRightMm,
    marginBottomMm,
    marginLeftMm,
    fontFamily,
    fontSizePx,
    backgroundColor,
    headerHtml,
    footerHtml,
    pageNumber,
    title
  } = options

  const width = toTwip(landscape ? pageHeightMm : pageWidthMm)
  const height = toTwip(landscape ? pageWidthMm : pageHeightMm)
  const hasHeader = !!headerHtml
  const hasFooter = !!footerHtml || !!pageNumber

  const result = (await HTMLtoDOCX(content, hasHeader ? headerHtml : null, {
    orientation: landscape ? 'landscape' : 'portrait',
    pageSize: { width, height },
    margins: {
      top: toTwip(marginTopMm),
      right: toTwip(marginRightMm),
      bottom: toTwip(marginBottomMm),
      left: toTwip(marginLeftMm),
      header: toTwip(Math.min(12.7, marginTopMm / 2)),
      footer: toTwip(Math.min(12.7, marginBottomMm / 2)),
      gutter: 0
    },
    title: title || undefined,
    creator: 'Markd',
    font: fontFamily || 'Calibri',
    // html-to-docx takes half-points; 1px = 0.75pt.
    fontSize: fontSizePx ? Math.round(fontSizePx * 0.75 * 2) : 22,
    header: hasHeader,
    footer: hasFooter,
    pageNumber: !!pageNumber,
    table: { row: { cantSplit: true } },
    lineNumber: false
  }, hasFooter ? footerHtml || '<p></p>' : null)) as Buffer | ArrayBuffer | Blob

  let buffer: Buffer
  if (Buffer.isBuffer(result)) {
    buffer = result
  } else if (result instanceof ArrayBuffer) {
    buffer = Buffer.from(result)
  } else {
    buffer = Buffer.from(await (result as Blob).arrayBuffer())
  }

  const pageColor = normalizeHex(backgroundColor)
  if (pageColor && pageColor !== 'FFFFFF') {
    buffer = await applyPageBackground(buffer, pageColor)
  }
  return buffer
}
