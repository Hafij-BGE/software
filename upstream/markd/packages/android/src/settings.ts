export type ThemeChoice = 'system' | 'word' | 'word-dark'
export type PageLayoutId = 'none' | 'a4' | 'letter' | 'legal'
/** `0` means fit the page to the screen width. */
export type ZoomLevel = number

export interface Settings {
  theme: ThemeChoice
  pageLayout: PageLayoutId
  /** Page margin in millimetres. */
  pageMargin: number
  zoom: ZoomLevel
  fontSize: number
  autoSave: boolean
  focusMode: boolean
  spellcheck: boolean
  /** Let Android turn stylus writing in the text into typed text (Android 14+). */
  penHandwriting: boolean
  /** Allow finger strokes in the sketch pad even after a pen was seen. */
  fingerDrawing: boolean
  sketchFormat: 'png' | 'svg'
  sketchPaper: boolean
  ribbonTab: string
  ribbonCollapsed: boolean
}

// An A4 sheet scaled to a phone's width leaves text too small to edit, so
// phones start in Mobile view and tablets in Print layout (A4).
const isTablet = Math.min(window.screen.width, window.screen.height) >= 600

const DEFAULTS: Settings = {
  theme: 'system',
  pageLayout: isTablet ? 'a4' : 'none',
  pageMargin: 25.4,
  zoom: 0,
  fontSize: 16,
  autoSave: true,
  focusMode: false,
  spellcheck: true,
  penHandwriting: true,
  fingerDrawing: false,
  sketchFormat: 'png',
  sketchPaper: true,
  ribbonTab: 'home',
  ribbonCollapsed: false
}

const STORAGE_KEY = 'markd.settings'

const load = (): Settings => {
  try {
    return { ...DEFAULTS, ...JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}') }
  } catch {
    return { ...DEFAULTS }
  }
}

export const settings: Settings = load()

type Listener = (changed: Partial<Settings>) => void
const listeners = new Set<Listener>()

export const updateSettings = (patch: Partial<Settings>): void => {
  Object.assign(settings, patch)
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
  } catch {
    // Storage can be unavailable; settings then last for this session only.
  }
  for (const listener of listeners) listener(patch)
}

export const onSettingsChange = (listener: Listener): void => {
  listeners.add(listener)
}
