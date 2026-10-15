// Sketch pad for stylus ink. Strokes are kept as vectors (position, pressure,
// tilt per sample) so the same outline geometry renders the live canvas and
// the exported PNG / SVG.
//
// Input rules:
// - A pen draws; its eraser end (buttons & 32) or barrel button (buttons & 2)
//   erases whole strokes.
// - Palm rejection: once a pen has been seen, touches are ignored unless
//   "finger drawing" is on, and never count while the pen is near the screen
//   (hovering or drawing). A touch stroke that started just before the pen
//   landed is dropped as a palm.
// - Pen hover shows a cursor preview sized by the current pressure / tilt.

import { ICONS } from './icons'
import { settings, updateSettings } from './settings'
import { reportPen } from './pen'
import { h, pushBack } from './ui'

interface Sample {
  x: number
  y: number
  /** 0..1 */
  p: number
  /** Stroke width multiplier from pen tilt; 1 when upright or unknown. */
  tilt: number
}

type Tool = 'pen' | 'highlighter' | 'eraser'

interface Stroke {
  tool: 'pen' | 'highlighter'
  color: string
  size: number
  samples: Sample[]
}

type Action =
  | { type: 'add', stroke: Stroke }
  | { type: 'erase', removed: Array<{ index: number, stroke: Stroke }> }
  | { type: 'clear', strokes: Stroke[] }

const COLORS = ['#1f1f1f', '#185abd', '#c50f1f', '#107c41', '#ca5010', '#7719aa', '#ffffff']
const HIGHLIGHTER_COLORS = ['#ffeb3b', '#7cf27c', '#5ee0ff', '#ff8ad8']
const SIZES = [2, 4, 8, 14]
/** A touch stroke shorter than this before a pen lands is treated as a palm. */
const PALM_WINDOW_MS = 400
/** How long after pen activity touches stay blocked. */
const PEN_PROXIMITY_MS = 600

const tiltFactor = (event: PointerEvent): number => {
  if (event.pointerType !== 'pen') return 1
  const altitude = (event as PointerEvent & { altitudeAngle?: number }).altitudeAngle
  let tiltDeg: number
  if (typeof altitude === 'number' && altitude > 0 && altitude < Math.PI / 2) {
    tiltDeg = 90 - (altitude * 180) / Math.PI
  } else {
    tiltDeg = Math.max(Math.abs(event.tiltX || 0), Math.abs(event.tiltY || 0))
  }
  // Up to 2x wider when the pen is laid down, like the side of a pencil lead.
  return 1 + Math.min(60, tiltDeg) / 60
}

const pressureOf = (event: PointerEvent): number => {
  // Mice and some touch screens report 0.5 (pressed) or 0; treat as medium.
  if (event.pointerType !== 'pen' || !event.pressure) return 0.5
  return Math.min(1, Math.max(0.05, event.pressure))
}

const widthAt = (stroke: Stroke, sample: Sample): number =>
  stroke.tool === 'highlighter'
    ? stroke.size * 3
    : stroke.size * (0.35 + 0.9 * sample.p) * sample.tilt

/**
 * Closed outline polygon around a variable-width stroke: offsets each sample
 * along the local normal by half its width, adds a round cap at the end, walks
 * back along the other side and closes with a round cap at the start.
 */
const outline = (stroke: Stroke): Array<[number, number]> => {
  const pts = stroke.samples
  const left: Array<[number, number]> = []
  const right: Array<[number, number]> = []
  const normals: number[] = []
  for (let i = 0; i < pts.length; i++) {
    const prev = pts[Math.max(0, i - 1)]!
    const next = pts[Math.min(pts.length - 1, i + 1)]!
    let dx = next.x - prev.x
    let dy = next.y - prev.y
    const len = Math.hypot(dx, dy) || 1
    dx /= len
    dy /= len
    const half = widthAt(stroke, pts[i]!) / 2
    left.push([pts[i]!.x - dy * half, pts[i]!.y + dx * half])
    right.push([pts[i]!.x + dy * half, pts[i]!.y - dx * half])
    normals.push(Math.atan2(dx, -dy))
  }
  // Half circle from `from` sweeping clockwise by PI around a sample.
  const cap = (sample: Sample, from: number): Array<[number, number]> => {
    const r = widthAt(stroke, sample) / 2
    const out: Array<[number, number]> = []
    for (let k = 1; k < 8; k++) {
      const angle = from - (Math.PI * k) / 8
      out.push([sample.x + Math.cos(angle) * r, sample.y + Math.sin(angle) * r])
    }
    return out
  }
  const last = pts.length - 1
  return [
    ...left,
    ...cap(pts[last]!, normals[last]!),
    ...right.reverse(),
    ...cap(pts[0]!, normals[0]! + Math.PI)
  ]
}

const traceStroke = (ctx: CanvasRenderingContext2D, stroke: Stroke): void => {
  const pts = stroke.samples
  ctx.beginPath()
  if (pts.length === 1) {
    ctx.arc(pts[0]!.x, pts[0]!.y, widthAt(stroke, pts[0]!) / 2, 0, Math.PI * 2)
    return
  }
  const poly = outline(stroke)
  ctx.moveTo(poly[0]![0], poly[0]![1])
  for (const [x, y] of poly) ctx.lineTo(x, y)
  ctx.closePath()
}

const paintStroke = (ctx: CanvasRenderingContext2D, stroke: Stroke): void => {
  ctx.save()
  ctx.fillStyle = stroke.color
  if (stroke.tool === 'highlighter') {
    ctx.globalAlpha = 0.4
    ctx.globalCompositeOperation = 'multiply'
  }
  traceStroke(ctx, stroke)
  ctx.fill('nonzero')
  ctx.restore()
}

const strokeBounds = (stroke: Stroke): { x0: number, y0: number, x1: number, y1: number } => {
  let x0 = Infinity
  let y0 = Infinity
  let x1 = -Infinity
  let y1 = -Infinity
  for (const s of stroke.samples) {
    const r = widthAt(stroke, s) / 2
    x0 = Math.min(x0, s.x - r)
    y0 = Math.min(y0, s.y - r)
    x1 = Math.max(x1, s.x + r)
    y1 = Math.max(y1, s.y + r)
  }
  return { x0, y0, x1, y1 }
}

const distToSegment = (px: number, py: number, a: Sample, b: Sample): number => {
  const dx = b.x - a.x
  const dy = b.y - a.y
  const len2 = dx * dx + dy * dy
  const t = len2 ? Math.max(0, Math.min(1, ((px - a.x) * dx + (py - a.y) * dy) / len2)) : 0
  return Math.hypot(px - (a.x + t * dx), py - (a.y + t * dy))
}

export interface SketchResult {
  blob: Blob
  format: 'png' | 'svg'
}

class SketchPad {
  private root: HTMLElement
  private base: HTMLCanvasElement
  private live: HTMLCanvasElement
  private baseCtx: CanvasRenderingContext2D
  private liveCtx: CanvasRenderingContext2D
  private strokes: Stroke[] = []
  private undoStack: Action[] = []
  private redoStack: Action[] = []
  private tool: Tool = 'pen'
  private color = COLORS[0]!
  private highlighterColor = HIGHLIGHTER_COLORS[0]!
  private size = SIZES[1]!
  private current: Stroke | null = null
  private currentPointer: { id: number, type: string, start: number, erasing: boolean } | null = null
  private erased: Array<{ index: number, stroke: Stroke }> = []
  private penSeen = false
  private penNearUntil = 0
  private hover: { x: number, y: number, r: number } | null = null
  private frame = 0
  private resolve: ((result: SketchResult | null) => void) | null = null
  private unregisterBack: (() => void) | null = null
  private toolbar: HTMLElement

  constructor() {
    this.base = h('canvas', { class: 'sketch-canvas' })
    this.live = h('canvas', { class: 'sketch-canvas live' })
    this.baseCtx = this.base.getContext('2d', { desynchronized: true })!
    this.liveCtx = this.live.getContext('2d', { desynchronized: true })!
    this.toolbar = h('div', { class: 'sketch-toolbar' })
    const surface = h('div', { class: 'sketch-surface' }, this.base, this.live)
    this.root = h('div', { class: 'sketch', hidden: true }, this.toolbar, surface)
    document.body.append(this.root)
    this.bindInput(this.live)
    new ResizeObserver(() => this.resize()).observe(surface)
  }

  open(): Promise<SketchResult | null> {
    this.strokes = []
    this.undoStack = []
    this.redoStack = []
    this.root.hidden = false
    this.root.classList.toggle('paper', settings.sketchPaper)
    this.renderToolbar()
    this.resize()
    this.unregisterBack = pushBack(() => this.finish(null))
    return new Promise((resolve) => {
      this.resolve = resolve
    })
  }

  private finish(result: SketchResult | null): void {
    this.unregisterBack?.()
    this.unregisterBack = null
    this.root.hidden = true
    this.resolve?.(result)
    this.resolve = null
  }

  private resize(): void {
    const surface = this.base.parentElement!
    const dpr = window.devicePixelRatio || 1
    for (const canvas of [this.base, this.live]) {
      canvas.width = Math.round(surface.clientWidth * dpr)
      canvas.height = Math.round(surface.clientHeight * dpr)
      canvas.getContext('2d')!.setTransform(dpr, 0, 0, dpr, 0, 0)
    }
    this.redrawBase()
    this.redrawLive()
  }

  private redrawBase(): void {
    const ctx = this.baseCtx
    ctx.clearRect(0, 0, this.base.width, this.base.height)
    for (const stroke of this.strokes) paintStroke(ctx, stroke)
  }

  private redrawLive(): void {
    this.frame = 0
    const ctx = this.liveCtx
    ctx.clearRect(0, 0, this.live.width, this.live.height)
    if (this.current) paintStroke(ctx, this.current)
    if (this.hover) {
      ctx.save()
      ctx.beginPath()
      ctx.arc(this.hover.x, this.hover.y, Math.max(2, this.hover.r), 0, Math.PI * 2)
      ctx.strokeStyle = this.tool === 'eraser' ? '#c50f1f' : '#185abd'
      ctx.lineWidth = 1
      ctx.setLineDash([3, 3])
      ctx.stroke()
      ctx.restore()
    }
  }

  private scheduleLive(): void {
    if (!this.frame) this.frame = requestAnimationFrame(() => this.redrawLive())
  }

  private point(event: PointerEvent): Sample {
    const rect = this.live.getBoundingClientRect()
    return { x: event.clientX - rect.left, y: event.clientY - rect.top, p: pressureOf(event), tilt: tiltFactor(event) }
  }

  private isEraserInput(event: PointerEvent): boolean {
    // 32 = pen eraser end, 2 = barrel button (Samsung S Pen side button).
    return this.tool === 'eraser' || (event.pointerType === 'pen' && ((event.buttons & 32) !== 0 || (event.buttons & 2) !== 0 || event.button === 5))
  }

  private touchBlocked(): boolean {
    return performance.now() < this.penNearUntil || (this.penSeen && !settings.fingerDrawing)
  }

  private bindInput(canvas: HTMLCanvasElement): void {
    canvas.addEventListener('pointerdown', (event) => {
      if (event.pointerType === 'pen') {
        this.penSeen = true
        this.penNearUntil = performance.now() + PEN_PROXIMITY_MS
        // A palm that touched down just before the pen: drop its stroke.
        if (this.currentPointer?.type === 'touch' && performance.now() - this.currentPointer.start < PALM_WINDOW_MS) {
          this.current = null
          this.currentPointer = null
        }
      } else if (event.pointerType === 'touch' && this.touchBlocked()) {
        return
      }
      if (this.currentPointer) return
      event.preventDefault()
      try {
        canvas.setPointerCapture(event.pointerId)
      } catch {
        // Pointer already gone (lifted between dispatch and handling).
      }
      const erasing = this.isEraserInput(event)
      this.currentPointer = { id: event.pointerId, type: event.pointerType, start: performance.now(), erasing }
      const sample = this.point(event)
      if (erasing) {
        this.erased = []
        this.eraseAt(sample.x, sample.y, sample.x, sample.y)
      } else {
        this.current = {
          tool: this.tool === 'highlighter' ? 'highlighter' : 'pen',
          color: this.tool === 'highlighter' ? this.highlighterColor : this.color,
          size: this.size,
          samples: [sample]
        }
      }
      this.updateHover(event)
      reportPen(event)
      this.scheduleLive()
    })

    canvas.addEventListener('pointermove', (event) => {
      if (event.pointerType === 'pen') this.penNearUntil = performance.now() + PEN_PROXIMITY_MS
      reportPen(event)
      if (!this.currentPointer || event.pointerId !== this.currentPointer.id) {
        if (event.pointerType === 'pen' || event.pointerType === 'mouse') this.updateHover(event)
        return
      }
      const events = event.getCoalescedEvents?.() ?? [event]
      for (const e of events.length ? events : [event]) {
        const sample = this.point(e)
        if (this.currentPointer.erasing) {
          const last = this.lastErasePoint ?? sample
          this.eraseAt(last.x, last.y, sample.x, sample.y)
          this.lastErasePoint = sample
        } else if (this.current) {
          const last = this.current.samples[this.current.samples.length - 1]!
          if (Math.hypot(sample.x - last.x, sample.y - last.y) >= 0.75) this.current.samples.push(sample)
        }
      }
      this.updateHover(event)
      this.scheduleLive()
    })

    const end = (event: PointerEvent): void => {
      if (!this.currentPointer || event.pointerId !== this.currentPointer.id) return
      if (this.currentPointer.erasing) {
        if (this.erased.length) this.commit({ type: 'erase', removed: this.erased })
        this.erased = []
        this.lastErasePoint = null
      } else if (this.current && event.type === 'pointerup') {
        this.strokes.push(this.current)
        paintStroke(this.baseCtx, this.current)
        this.commit({ type: 'add', stroke: this.current })
      }
      this.current = null
      this.currentPointer = null
      if (event.pointerType !== 'pen') this.hover = null
      this.scheduleLive()
    }
    canvas.addEventListener('pointerup', end)
    canvas.addEventListener('pointercancel', end)
    canvas.addEventListener('pointerleave', (event) => {
      if (!this.currentPointer) {
        this.hover = null
        this.scheduleLive()
      }
      reportPen(event)
    })
    // Keep long-press / context menus away from the drawing surface.
    canvas.addEventListener('contextmenu', (event) => event.preventDefault())
  }

  private lastErasePoint: Sample | null = null

  private updateHover(event: PointerEvent): void {
    if (event.pointerType === 'touch') {
      this.hover = null
      return
    }
    const sample = this.point(event)
    const eraser = this.isEraserInput(event)
    const probe: Stroke = { tool: this.tool === 'highlighter' ? 'highlighter' : 'pen', color: '', size: this.size, samples: [] }
    this.hover = { x: sample.x, y: sample.y, r: eraser ? this.eraserRadius() : widthAt(probe, sample) / 2 }
    this.scheduleLive()
  }

  private eraserRadius(): number {
    return Math.max(8, this.size * 3)
  }

  private eraseAt(x0: number, y0: number, x1: number, y1: number): void {
    const radius = this.eraserRadius()
    const a: Sample = { x: x0, y: y0, p: 0, tilt: 1 }
    const b: Sample = { x: x1, y: y1, p: 0, tilt: 1 }
    let changed = false
    for (let i = this.strokes.length - 1; i >= 0; i--) {
      const stroke = this.strokes[i]!
      const box = strokeBounds(stroke)
      if (Math.max(x0, x1) + radius < box.x0 || Math.min(x0, x1) - radius > box.x1 ||
        Math.max(y0, y1) + radius < box.y0 || Math.min(y0, y1) - radius > box.y1) continue
      const hit = stroke.samples.some((s) => distToSegment(s.x, s.y, a, b) <= radius + widthAt(stroke, s) / 2)
      if (hit) {
        this.erased.push({ index: i, stroke })
        this.strokes.splice(i, 1)
        changed = true
      }
    }
    if (changed) this.redrawBase()
  }

  private commit(action: Action): void {
    this.undoStack.push(action)
    this.redoStack = []
    this.renderToolbar()
  }

  private undo(): void {
    const action = this.undoStack.pop()
    if (!action) return
    if (action.type === 'add') this.strokes.splice(this.strokes.lastIndexOf(action.stroke), 1)
    else if (action.type === 'erase') {
      // Re-insert in the reverse order of removal so indices line up.
      for (const { index, stroke } of [...action.removed].reverse()) this.strokes.splice(index, 0, stroke)
    } else this.strokes = action.strokes
    this.redoStack.push(action)
    this.redrawBase()
    this.renderToolbar()
  }

  private redo(): void {
    const action = this.redoStack.pop()
    if (!action) return
    if (action.type === 'add') this.strokes.push(action.stroke)
    else if (action.type === 'erase') {
      for (const { stroke } of action.removed) this.strokes.splice(this.strokes.indexOf(stroke), 1)
    } else this.strokes = []
    this.undoStack.push(action)
    this.redrawBase()
    this.renderToolbar()
  }

  private clear(): void {
    if (!this.strokes.length) return
    this.commit({ type: 'clear', strokes: this.strokes })
    this.strokes = []
    this.redrawBase()
  }

  private bounds(): { x: number, y: number, w: number, h: number } | null {
    if (!this.strokes.length) return null
    let x0 = Infinity
    let y0 = Infinity
    let x1 = -Infinity
    let y1 = -Infinity
    for (const stroke of this.strokes) {
      const box = strokeBounds(stroke)
      x0 = Math.min(x0, box.x0)
      y0 = Math.min(y0, box.y0)
      x1 = Math.max(x1, box.x1)
      y1 = Math.max(y1, box.y1)
    }
    const pad = 12
    return { x: Math.floor(x0 - pad), y: Math.floor(y0 - pad), w: Math.ceil(x1 - x0 + 2 * pad), h: Math.ceil(y1 - y0 + 2 * pad) }
  }

  private async toPng(): Promise<Blob | null> {
    const box = this.bounds()
    if (!box) return null
    const scale = 2
    const canvas = document.createElement('canvas')
    canvas.width = box.w * scale
    canvas.height = box.h * scale
    const ctx = canvas.getContext('2d')!
    if (settings.sketchPaper) {
      ctx.fillStyle = '#ffffff'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
    }
    ctx.setTransform(scale, 0, 0, scale, -box.x * scale, -box.y * scale)
    for (const stroke of this.strokes) paintStroke(ctx, stroke)
    return new Promise((resolve) => canvas.toBlob(resolve, 'image/png'))
  }

  private toSvg(): Blob | null {
    const box = this.bounds()
    if (!box) return null
    const r = (n: number): string => n.toFixed(1)
    const paths = this.strokes.map((stroke) => {
      const pts = stroke.samples
      let d: string
      if (pts.length === 1) {
        const rad = widthAt(stroke, pts[0]!) / 2
        d = `M${r(pts[0]!.x - rad)} ${r(pts[0]!.y)}a${r(rad)} ${r(rad)} 0 1 0 ${r(2 * rad)} 0a${r(rad)} ${r(rad)} 0 1 0 ${r(-2 * rad)} 0Z`
      } else {
        d = `M${outline(stroke).map(([x, y]) => `${r(x)} ${r(y)}`).join('L')}Z`
      }
      const extra = stroke.tool === 'highlighter' ? ' fill-opacity="0.4" style="mix-blend-mode:multiply"' : ''
      return `<path d="${d}" fill="${stroke.color}"${extra}/>`
    })
    const background = settings.sketchPaper ? `<rect x="${box.x}" y="${box.y}" width="${box.w}" height="${box.h}" fill="#ffffff"/>` : ''
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${box.x} ${box.y} ${box.w} ${box.h}" width="${box.w}" height="${box.h}">${background}${paths.join('')}</svg>`
    return new Blob([svg], { type: 'image/svg+xml' })
  }

  private async insert(): Promise<void> {
    const format = settings.sketchFormat
    const blob = format === 'svg' ? this.toSvg() : await this.toPng()
    this.finish(blob ? { blob, format } : null)
  }

  private renderToolbar(): void {
    const tb = this.toolbar
    tb.replaceChildren()
    const toolButton = (tool: Tool, icon: string, label: string): HTMLElement =>
      h('button', {
        type: 'button',
        class: `tool${this.tool === tool ? ' active' : ''}`,
        'aria-label': label,
        'aria-pressed': String(this.tool === tool),
        title: label,
        html: icon,
        onClick: () => {
          this.tool = tool
          this.renderToolbar()
        }
      })
    const palette = this.tool === 'highlighter' ? HIGHLIGHTER_COLORS : COLORS
    const activeColor = this.tool === 'highlighter' ? this.highlighterColor : this.color
    tb.append(
      h('button', { type: 'button', class: 'btn', onClick: () => this.finish(null) }, 'Cancel'),
      h('div', { class: 'sketch-group' },
        toolButton('pen', ICONS.pen(24), 'Pen'),
        toolButton('highlighter', ICONS.highlighter(24), 'Highlighter'),
        toolButton('eraser', ICONS.eraser(24), 'Stroke eraser')
      ),
      h('div', { class: 'sketch-group colors' },
        ...palette.map((color) => h('button', {
          type: 'button',
          class: `swatch${color === activeColor && this.tool !== 'eraser' ? ' active' : ''}`,
          style: `--swatch:${color}`,
          'aria-label': `Colour ${color}`,
          onClick: () => {
            if (this.tool === 'highlighter') this.highlighterColor = color
            else {
              this.color = color
              if (this.tool === 'eraser') this.tool = 'pen'
            }
            this.renderToolbar()
          }
        }))
      ),
      h('div', { class: 'sketch-group sizes' },
        ...SIZES.map((size) => h('button', {
          type: 'button',
          class: `size${size === this.size ? ' active' : ''}`,
          'aria-label': `Size ${size}`,
          html: `<span style="width:${Math.min(18, size + 2)}px;height:${Math.min(18, size + 2)}px"></span>`,
          onClick: () => {
            this.size = size
            this.renderToolbar()
          }
        }))
      ),
      h('div', { class: 'sketch-group' },
        h('button', { type: 'button', class: 'tool', 'aria-label': 'Undo', title: 'Undo', html: ICONS.undo, disabled: !this.undoStack.length, onClick: () => this.undo() }),
        h('button', { type: 'button', class: 'tool', 'aria-label': 'Redo', title: 'Redo', html: ICONS.redo, disabled: !this.redoStack.length, onClick: () => this.redo() }),
        h('button', { type: 'button', class: 'tool', 'aria-label': 'Clear', title: 'Clear', html: ICONS.trash(22), onClick: () => this.clear() })
      ),
      h('div', { class: 'sketch-group' },
        h('button', {
          type: 'button',
          class: `tool${settings.fingerDrawing ? ' active' : ''}`,
          'aria-label': 'Draw with finger',
          'aria-pressed': String(settings.fingerDrawing),
          title: 'Draw with finger (off = palm rejection while using a pen)',
          html: ICONS.hand(24),
          onClick: () => {
            updateSettings({ fingerDrawing: !settings.fingerDrawing })
            this.renderToolbar()
          }
        }),
        h('button', {
          type: 'button',
          class: 'btn small',
          title: 'Image format',
          onClick: () => {
            updateSettings({ sketchFormat: settings.sketchFormat === 'png' ? 'svg' : 'png' })
            this.renderToolbar()
          }
        }, settings.sketchFormat.toUpperCase())
      ),
      h('button', { type: 'button', class: 'btn primary', disabled: !this.strokes.length, onClick: () => this.insert() }, 'Insert')
    )
  }
}

let pad: SketchPad | null = null

export const openSketch = (): Promise<SketchResult | null> => {
  pad ??= new SketchPad()
  return pad.open()
}
