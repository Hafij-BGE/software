// Stylus status for the status bar: hover, pressure, tilt and eraser state.
// Listeners are passive and never cancel events, so Android's stylus
// handwriting (pen writing into the text) keeps working in the editor.

let label: HTMLElement | null = null
let clearTimer = 0

const tiltDegrees = (event: PointerEvent): number | null => {
  const altitude = (event as PointerEvent & { altitudeAngle?: number }).altitudeAngle
  if (typeof altitude === 'number' && altitude > 0 && altitude < Math.PI / 2) {
    return Math.round(90 - (altitude * 180) / Math.PI)
  }
  if (event.tiltX || event.tiltY) return Math.round(Math.max(Math.abs(event.tiltX), Math.abs(event.tiltY)))
  return null
}

export const reportPen = (event: PointerEvent): void => {
  if (event.pointerType !== 'pen' || !label) return
  const parts: string[] = []
  const eraser = (event.buttons & 32) !== 0
  const barrel = (event.buttons & 2) !== 0
  if (event.type === 'pointerleave' || event.type === 'pointerout') parts.push('Pen away')
  else if (event.buttons === 0) parts.push('Pen hover')
  else parts.push(eraser ? 'Eraser' : barrel ? 'Pen (button)' : `Pen ${Math.round(event.pressure * 100)}%`)
  const tilt = tiltDegrees(event)
  if (tilt !== null && event.type !== 'pointerleave') parts.push(`tilt ${tilt}°`)
  label.textContent = parts.join(' · ')
  label.hidden = false
  window.clearTimeout(clearTimer)
  clearTimer = window.setTimeout(() => {
    if (label) label.hidden = true
  }, 4000)
}

export const initPenStatus = (): void => {
  label = document.getElementById('status-pen')
  for (const type of ['pointerdown', 'pointermove', 'pointerup', 'pointerleave'] as const) {
    document.addEventListener(type, (event) => {
      if (event.pointerType === 'pen') reportPen(event)
    }, { passive: true, capture: true })
  }
}
