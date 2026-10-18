// Zooming the paper with Ctrl+wheel, a touchpad pinch (Chromium and WebView2 report one as
// Ctrl+wheel) and Ctrl+= / Ctrl+- / Ctrl+0, instead of letting the web view scale the whole
// window, ribbon included. A desktop host that sees the pinch itself (WebKitGTK) calls
// `window.__markdZoomBy`.

import { updateSettings } from './settings'
import { currentZoom } from './theme'

const MIN = 0.25
const MAX = 4
const STEPS = [0.25, 0.33, 0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4]

declare global {
  interface Window {
    __markdZoomBy?: (factor: number, clientX?: number, clientY?: number) => void
  }
}

let target = 0
let anchor: { x: number, y: number } | null = null
let frame = 0

/** Applies the newest requested zoom once per frame, keeping the point under the pointer still. */
const flush = (): void => {
  frame = 0
  const desk = document.getElementById('desk')
  const before = currentZoom()
  if (!desk || !target || Math.abs(target - before) < 0.001) return
  const rect = desk.getBoundingClientRect()
  const x = anchor ? anchor.x - rect.left : desk.clientWidth / 2
  const y = anchor ? anchor.y - rect.top : desk.clientHeight / 2
  const left = desk.scrollLeft + x
  const top = desk.scrollTop + y
  updateSettings({ zoom: Math.round(target * 1000) / 1000 })
  const ratio = currentZoom() / before
  desk.scrollLeft = left * ratio - x
  desk.scrollTop = top * ratio - y
}

const zoomTo = (zoom: number, x?: number, y?: number): void => {
  target = Math.min(MAX, Math.max(MIN, zoom))
  anchor = x === undefined || y === undefined ? null : { x, y }
  if (!frame) frame = requestAnimationFrame(flush)
}

/** The zoom a new request starts from: one still waiting for its frame, else the shown one. */
const base = (): number => (frame && target ? target : currentZoom())

export const zoomBy = (factor: number, x?: number, y?: number): void => {
  if (Number.isFinite(factor) && factor > 0) zoomTo(base() * factor, x, y)
}

const step = (direction: 1 | -1): void => {
  const now = base()
  const next = direction > 0 ? STEPS.find((s) => s > now + 0.001) : [...STEPS].reverse().find((s) => s < now - 0.001)
  zoomTo(next ?? (direction > 0 ? MAX : MIN))
}

export const initZoom = (): void => {
  window.__markdZoomBy = zoomBy

  window.addEventListener('wheel', (event) => {
    if (!event.ctrlKey) return
    event.preventDefault()
    // Lines and pages to pixels; a wheel notch is about 100px, a pinch a few px per event.
    const unit = event.deltaMode === 1 ? 40 : event.deltaMode === 2 ? 800 : 1
    const delta = Math.max(-200, Math.min(200, event.deltaY * unit))
    zoomBy(Math.exp(-delta * 0.002), event.clientX, event.clientY)
  }, { passive: false, capture: true })

  document.addEventListener('keydown', (event) => {
    if (event.defaultPrevented || event.isComposing || event.altKey || !(event.ctrlKey || event.metaKey)) return
    let run: (() => void) | null = null
    if (event.key === '=' || event.key === '+') run = () => step(1)
    else if (event.key === '-' || event.key === '_') run = () => step(-1)
    else if (event.key === '0' && !event.shiftKey) run = () => zoomTo(1)
    if (!run) return
    event.preventDefault()
    event.stopPropagation()
    run()
  }, true)
}
