// Prepare exported HTML for the native DOCX converter (main process).
//
// Word knows nothing about CSS classes or variables, so the styled export
// document is laid out in an off-screen iframe and every element's *computed*
// style (which already reflects the chosen editor/export theme) is written
// back as an inline `style` attribute. Diagrams (SVG) are rasterised to PNG and
// KaTeX math is replaced by its TeX source, because Word cannot embed either.

const BLOCK_STYLE_PROPS = ['color', 'background-color', 'text-align', 'font-family', 'font-size'] as const

const INLINE_STYLE_PROPS = [
  'color',
  'background-color',
  'font-family',
  'font-size',
  'font-weight',
  'font-style',
  'text-decoration-line'
] as const

const TRANSPARENT = /^(transparent|rgba\(\s*0,\s*0,\s*0,\s*0\s*\))$/i

const toHex = (color: string): string | null => {
  const match = /rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\s*\)/i.exec(color)
  if (!match) return /^#[0-9a-f]{6}$/i.test(color) ? color : null
  const alpha = match[4] === undefined ? 1 : Number(match[4])
  if (alpha === 0) return null
  return (
    '#' +
    [match[1], match[2], match[3]]
      .map((v) => Number(v).toString(16).padStart(2, '0'))
      .join('')
  )
}

/** Blend a translucent colour onto the page colour; Word has no alpha. */
const flatten = (color: string, pageBg: [number, number, number]): string | null => {
  const match = /rgba?\(\s*(\d+),\s*(\d+),\s*(\d+)(?:,\s*([\d.]+))?\s*\)/i.exec(color)
  if (!match) return toHex(color)
  const alpha = match[4] === undefined ? 1 : Number(match[4])
  if (alpha === 0) return null
  const rgb = [match[1], match[2], match[3]].map((v, i) =>
    Math.round(Number(v) * alpha + pageBg[i]! * (1 - alpha))
  )
  return '#' + rgb.map((v) => v.toString(16).padStart(2, '0')).join('')
}

// Fallback/notation fonts the exporter prepends (or CSS generic keywords) that
// mean nothing to Word; skip them and use the first real family instead.
const NON_WORD_FONTS = new Set([
  'noto music',
  'noto sans math',
  'noto color emoji',
  'apple color emoji',
  'segoe ui emoji',
  'segoe ui symbol',
  '-apple-system',
  'blinkmacsystemfont',
  'system-ui',
  'ui-sans-serif',
  'ui-serif',
  'ui-monospace',
  'sans-serif',
  'serif',
  'monospace',
  'cursive',
  'fantasy',
  'emoji',
  'math'
])
const MONO_KEYWORDS = /monospace|mono\b|ui-monospace/i

const firstFontFamily = (value: string): string => {
  const families = value
    .split(',')
    .map((family) => family.trim().replace(/^["']|["']$/g, ''))
    .filter(Boolean)
  const real = families.find((family) => !NON_WORD_FONTS.has(family.toLowerCase()))
  if (real) return real
  return MONO_KEYWORDS.test(value) ? 'Consolas' : ''
}

const pxToPt = (value: string): string | null => {
  const px = parseFloat(value)
  return Number.isFinite(px) && px > 0 ? `${Math.round(px * 0.75 * 2) / 2}pt` : null
}

const loadIframe = (html: string): Promise<HTMLIFrameElement> =>
  new Promise((resolve, reject) => {
    const iframe = document.createElement('iframe')
    iframe.setAttribute('aria-hidden', 'true')
    iframe.style.cssText =
      'position:fixed;left:-20000px;top:0;width:1000px;height:1000px;border:0;visibility:hidden;'
    iframe.onload = () => resolve(iframe)
    iframe.onerror = () => reject(new Error('Unable to lay out document for DOCX export.'))
    iframe.srcdoc = html
    document.body.appendChild(iframe)
  })

const svgToPng = (svg: SVGSVGElement, scale = 2): Promise<string | null> =>
  new Promise((resolve) => {
    try {
      const rect = svg.getBoundingClientRect()
      const width = Math.max(1, Math.ceil(rect.width || Number(svg.getAttribute('width')) || 600))
      const height = Math.max(1, Math.ceil(rect.height || Number(svg.getAttribute('height')) || 400))
      const clone = svg.cloneNode(true) as SVGSVGElement
      clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')
      clone.setAttribute('width', String(width))
      clone.setAttribute('height', String(height))
      const data = new XMLSerializer().serializeToString(clone)
      const img = new Image()
      img.onload = () => {
        try {
          const canvas = document.createElement('canvas')
          canvas.width = width * scale
          canvas.height = height * scale
          const ctx = canvas.getContext('2d')
          if (!ctx) return resolve(null)
          ctx.scale(scale, scale)
          ctx.drawImage(img, 0, 0, width, height)
          resolve(canvas.toDataURL('image/png'))
        } catch {
          resolve(null)
        }
      }
      img.onerror = () => resolve(null)
      img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(data)}`
    } catch {
      resolve(null)
    }
  })

export interface DocxHtmlResult {
  /** Body HTML with inline styles, ready for html-to-docx. */
  html: string
  /** Resolved page background colour (`#rrggbb`). */
  backgroundColor: string
}

/**
 * Lay out a full styled export document and return its article markup with
 * computed styles inlined.
 */
export const prepareDocxHtml = async(fullHtml: string, contentWidthPx: number): Promise<DocxHtmlResult> => {
  const iframe = await loadIframe(fullHtml)
  try {
    const doc = iframe.contentDocument
    if (!doc) throw new Error('Unable to lay out document for DOCX export.')
    iframe.style.width = `${Math.max(320, Math.round(contentWidthPx))}px`
    await doc.fonts?.ready

    const article = (doc.querySelector('article.markdown-body') || doc.body) as HTMLElement
    const win = doc.defaultView!
    const bodyStyle = win.getComputedStyle(article)
    const pageBgHex =
      toHex(bodyStyle.backgroundColor) || toHex(win.getComputedStyle(doc.body).backgroundColor) || '#ffffff'
    const pageBg: [number, number, number] = [1, 3, 5].map((i) =>
      parseInt(pageBgHex.slice(i, i + 2), 16)
    ) as [number, number, number]

    // Rasterise diagrams before styles are baked (needs live layout).
    const svgs = Array.from(article.querySelectorAll('svg')).filter(
      (svg) => !svg.closest('.katex') && !svg.parentElement?.closest('svg')
    )
    for (const svg of svgs) {
      const png = await svgToPng(svg as SVGSVGElement)
      const replacement = doc.createElement(png ? 'img' : 'p')
      if (png) {
        const rect = svg.getBoundingClientRect()
        replacement.setAttribute('src', png)
        replacement.setAttribute('width', String(Math.round(Math.min(rect.width, contentWidthPx))))
      } else {
        replacement.textContent = '[diagram]'
      }
      svg.replaceWith(replacement)
    }

    // KaTeX: Word cannot render it, keep the TeX source instead.
    for (const math of Array.from(article.querySelectorAll('.katex-display, .katex'))) {
      if (!math.isConnected) continue
      const tex = math.querySelector('annotation[encoding="application/x-tex"]')?.textContent?.trim()
      const code = doc.createElement('code')
      code.textContent = tex ? (math.classList.contains('katex-display') ? `$$${tex}$$` : `$${tex}$`) : math.textContent || ''
      const host = math.classList.contains('katex-display') ? doc.createElement('p') : null
      if (host) {
        host.appendChild(code)
        math.replaceWith(host)
      } else {
        math.replaceWith(code)
      }
    }

    // Front matter / hidden helpers should not reach Word.
    for (const el of Array.from(article.querySelectorAll<HTMLElement>('*'))) {
      if (win.getComputedStyle(el).display === 'none') el.remove()
    }

    for (const el of Array.from(article.querySelectorAll<HTMLElement>('*'))) {
      const cs = win.getComputedStyle(el)
      const isBlock = /^(block|list-item|table|flex|grid)$/.test(cs.display)
      const props = isBlock ? [...BLOCK_STYLE_PROPS, 'font-weight', 'font-style'] : INLINE_STYLE_PROPS
      const decl: string[] = []
      for (const prop of props) {
        const value = cs.getPropertyValue(prop)
        if (!value) continue
        if (prop === 'color') {
          const hex = flatten(value, pageBg)
          if (hex) decl.push(`color:${hex}`)
        } else if (prop === 'background-color') {
          if (TRANSPARENT.test(value)) continue
          const hex = flatten(value, pageBg)
          if (hex && hex.toLowerCase() !== pageBgHex.toLowerCase()) decl.push(`background-color:${hex}`)
        } else if (prop === 'font-size') {
          const pt = pxToPt(value)
          if (pt) decl.push(`font-size:${pt}`)
        } else if (prop === 'font-family') {
          const family = firstFontFamily(value)
          if (family) decl.push(`font-family:${family}`)
        } else if (prop === 'font-weight') {
          if (Number(value) >= 600 || value === 'bold') decl.push('font-weight:bold')
        } else if (prop === 'font-style') {
          if (value === 'italic') decl.push('font-style:italic')
        } else if (prop === 'text-decoration-line') {
          if (value !== 'none') decl.push(`text-decoration:${value}`)
        } else if (prop === 'text-align') {
          if (/^(center|right|justify)$/.test(value)) decl.push(`text-align:${value}`)
        }
      }
      // Blockquotes: keep the coloured bar as a left border.
      if (el.tagName === 'BLOCKQUOTE') {
        const bar = flatten(cs.borderLeftColor, pageBg)
        if (bar) decl.push(`border-left:3px solid ${bar}`, 'padding-left:8px')
      }
      if (el.tagName === 'TD' || el.tagName === 'TH') {
        const border = flatten(cs.borderTopColor, pageBg)
        if (border) decl.push(`border:1px solid ${border}`)
      }
      el.removeAttribute('class')
      el.removeAttribute('id')
      if (decl.length) el.setAttribute('style', decl.join(';'))
      else el.removeAttribute('style')
    }

    // Task-list checkboxes have no Word equivalent through html-to-docx.
    for (const box of Array.from(article.querySelectorAll('input[type="checkbox"]'))) {
      box.replaceWith(doc.createTextNode((box as HTMLInputElement).checked ? '☑ ' : '☐ '))
    }

    return { html: article.innerHTML, backgroundColor: pageBgHex }
  } finally {
    iframe.remove()
  }
}
