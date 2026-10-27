// Inkwell integration. Inkwell (android/inkwell) is built into the app: Sketch
// opens its canvas, and the home screen (HomeActivity) lists every file. Inkwell
// returns an SVG that carries the
// whole note (gzip + base64 in <metadata><inkwell:note>), so selecting such an
// image later offers "Edit in Inkwell", which reopens the note and replaces the
// image with the new result.

import { docAssetUrl, getMuya, notifyChange } from './editor'
import { storeImageBlob } from './document'
import { callNative, isCancelled, isNative } from './native'
import { settings } from './settings'
import { h, toast } from './ui'

const NOTE = /<inkwell:note[^>]*>([A-Za-z0-9+/=\s]+)<\/inkwell:note>/

/** Back to the home screen listing Markdown files, drawings and PDFs. */
export const openHome = (): void => {
  if (!isNative()) {
    toast('The home screen is available in the Android app')
    return
  }
  callNative('openHome').catch((err: Error) => toast(err.message))
}

export const inkwellAvailable = async(): Promise<boolean> => {
  if (!isNative()) return false
  try {
    return await callNative<boolean>('inkwellAvailable')
  } catch {
    return false
  }
}

/**
 * Open Inkwell, optionally on the note embedded in `svg`. Resolves to the new
 * SVG, or null when the user backs out.
 */
export const drawInInkwell = async(svg?: string): Promise<Blob | null> => {
  const ink = svg ? NOTE.exec(svg)?.[1]?.replace(/\s/g, '') ?? '' : ''
  try {
    const result = await callNative<{ svg: string }>('inkwellSketch', { ink, paper: settings.sketchPaper })
    return new Blob([result.svg], { type: 'image/svg+xml' })
  } catch (err) {
    if (isCancelled(err)) return null
    throw err
  }
}

// ---- Editing an inserted sketch ----

interface ImageInfo {
  token: { attrs: Record<string, string | undefined> }
}

interface ImageBlock {
  replaceImage: (info: ImageInfo, image: { alt?: string, src?: string, title?: string }) => void
}

let chip: HTMLElement | null = null
let hideTimer = 0

const hideChip = (): void => {
  window.clearTimeout(hideTimer)
  chip?.remove()
  chip = null
}

/** Absolute URL for an image src as written in the Markdown. */
const resolveSrc = (src: string): string | null => {
  if (/^data:image\/svg\+xml/i.test(src)) return src
  if (/^[a-z][a-z0-9+.-]*:/i.test(src)) return null
  try {
    return docAssetUrl(src.split('/').map((part) => decodeURIComponent(part)).join('/'))
  } catch {
    return docAssetUrl(src)
  }
}

const readSvg = async(src: string): Promise<string | null> => {
  const url = resolveSrc(src)
  if (!url) return null
  try {
    const response = await fetch(url)
    if (!response.ok) return null
    const text = await response.text()
    return NOTE.test(text) ? text : null
  } catch {
    return null
  }
}

const showChip = (block: ImageBlock, info: ImageInfo, svg: string): void => {
  hideChip()
  chip = h('button', {
    type: 'button',
    class: 'inkwell-chip',
    // Act on pointerdown: the tap would otherwise blur muya's image selection
    // first, which hides this chip before a click arrives.
    onPointerdown: async(event: PointerEvent) => {
      event.preventDefault()
      hideChip()
      try {
        const blob = await drawInInkwell(svg)
        if (!blob) return
        const src = await storeImageBlob(blob, 'sketch')
        const { alt, title } = info.token.attrs
        block.replaceImage(info, { alt: alt || 'Sketch', src, title: title ?? '' })
        notifyChange()
        toast('Sketch updated')
      } catch (err) {
        toast(`Inkwell: ${(err as Error).message}`)
      }
    }
  }, 'Edit in Inkwell')
  document.body.append(chip)
  hideTimer = window.setTimeout(hideChip, 8000)
}

/** Watch image selection for sketches made in Inkwell. */
export const initInkwellEditing = (): void => {
  if (!isNative()) return
  const muya = getMuya() as unknown as {
    eventCenter: { on: (name: string, cb: (payload: { block: ImageBlock, reference: unknown, imageInfo: ImageInfo }) => void) => void }
  }
  muya.eventCenter.on('muya-image-toolbar', async({ block, reference, imageInfo }) => {
    if (!reference || !imageInfo) {
      hideChip()
      return
    }
    const src = imageInfo.token.attrs.src ?? ''
    if (!/\.svg(\?|#|$)/i.test(src) && !/^data:image\/svg\+xml/i.test(src)) {
      hideChip()
      return
    }
    const svg = await readSvg(src)
    if (svg && await inkwellAvailable()) showChip(block, imageInfo, svg)
  })
}
