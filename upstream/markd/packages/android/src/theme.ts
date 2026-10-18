// Word Light / Word Dark themes shared with the desktop app, plus the A4 /
// Letter / Legal page view (the Android take on desktop `setPageLayout`).

import wordCss from '../../desktop/src/renderer/src/assets/themes/word.theme.css?raw'
import wordDarkCss from '../../desktop/src/renderer/src/assets/themes/word-dark.theme.css?raw'
import prismWordCss from '../../desktop/src/renderer/src/assets/themes/prismjs/word.theme.css?raw'
import prismWordDarkCss from '../../desktop/src/renderer/src/assets/themes/prismjs/word-dark.theme.css?raw'
import { callNative, isNative } from './native'
import { onSettingsChange, settings, type PageLayoutId } from './settings'

export interface PageDimensions {
  /** Width in millimetres. */
  width: number
  /** Height in millimetres. */
  height: number
  printName: 'A4' | 'Letter' | 'Legal'
}

export const PAGE_DIMENSIONS: Readonly<Record<Exclude<PageLayoutId, 'none'>, PageDimensions>> = Object.freeze({
  a4: { width: 210, height: 297, printName: 'A4' },
  letter: { width: 215.9, height: 279.4, printName: 'Letter' },
  legal: { width: 215.9, height: 355.6, printName: 'Legal' }
})

export const getPageDimensions = (layout: string): PageDimensions | null =>
  layout in PAGE_DIMENSIONS ? PAGE_DIMENSIONS[layout as keyof typeof PAGE_DIMENSIONS] : null

export const MM_TO_PX = 96 / 25.4

const darkQuery = window.matchMedia('(prefers-color-scheme: dark)')

export const isDarkTheme = (): boolean =>
  settings.theme === 'word-dark' || (settings.theme === 'system' && darkQuery.matches)

export const themeCss = (): string => (isDarkTheme() ? wordDarkCss : wordCss)

const styleElement = (id: string): HTMLStyleElement => {
  let el = document.getElementById(id) as HTMLStyleElement | null
  if (!el) {
    el = document.createElement('style')
    el.id = id
  }
  // Keep theme sheets last so their :root variables override muya's defaults.
  document.head.appendChild(el)
  return el
}

type ThemeListener = (dark: boolean) => void
const themeListeners = new Set<ThemeListener>()
export const onThemeChange = (listener: ThemeListener): void => {
  themeListeners.add(listener)
}

export const applyTheme = (): void => {
  const dark = isDarkTheme()
  styleElement('markd-theme').textContent = dark ? wordDarkCss : wordCss
  styleElement('markd-prism-theme').textContent = dark ? prismWordDarkCss : prismWordCss
  document.documentElement.dataset.theme = dark ? 'dark' : 'light'
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
  if (isNative()) {
    const css = getComputedStyle(document.documentElement)
    callNative('setChrome', {
      top: css.getPropertyValue('--wordTitleBarColor').trim() || '#185abd',
      bottom: css.getPropertyValue('--appStatusBg').trim() || '#185abd',
      dark
    }).catch(() => {})
  }
  for (const listener of themeListeners) listener(dark)
}

const pageWidthPx = (): number => {
  const page = getPageDimensions(settings.pageLayout)
  return page ? page.width * MM_TO_PX : 0
}

/** Scale the paper so it fits the desk (zoom `0`) or to a fixed zoom level. */
export const applyZoom = (): void => {
  const desk = document.getElementById('desk')
  const sheet = document.getElementById('page')
  if (!desk || !sheet) return
  const width = pageWidthPx()
  // A set zoom applies with or without pages; "fit" (0) fits the page's width, or is 100%.
  let zoom = 1
  if (settings.zoom > 0) zoom = settings.zoom
  else if (width) zoom = Math.min(1.5, (desk.clientWidth - 16) / width)
  sheet.style.zoom = String(Math.max(0.2, zoom))
  document.dispatchEvent(new CustomEvent('markd:zoom', { detail: zoom }))
}

export const currentZoom = (): number => Number(document.getElementById('page')?.style.zoom || 1)

export const applyPageLayout = (): void => {
  const page = getPageDimensions(settings.pageLayout)
  document.body.classList.toggle('page-layout', !!page)
  const margin = settings.pageMargin >= 5 && settings.pageMargin <= 51 ? settings.pageMargin : 25.4
  const root = document.documentElement.style
  if (page) {
    root.setProperty('--page-width', `${page.width}mm`)
    root.setProperty('--page-height', `${page.height}mm`)
    root.setProperty('--page-margin', `${margin}mm`)
  }
  applyZoom()
}

export const initTheme = (): void => {
  applyTheme()
  applyPageLayout()
  darkQuery.addEventListener('change', () => {
    if (settings.theme === 'system') applyTheme()
  })
  onSettingsChange((patch) => {
    if ('theme' in patch) applyTheme()
    if ('pageLayout' in patch || 'pageMargin' in patch) applyPageLayout()
    else if ('zoom' in patch) applyZoom()
  })
  window.addEventListener('resize', () => applyZoom())
}
