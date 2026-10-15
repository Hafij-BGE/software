import {
  THEME_STYLE_ID,
  COMMON_STYLE_ID,
  DEFAULT_CODE_FONT_FAMILY,
  oneDarkThemes,
  railscastsThemes
} from '../config'
import {
  dark,
  graphite,
  materialDark,
  oneDark,
  ulysses,
  // New gogh themes - Dark
  dracula,
  nord,
  catppuccinMocha,
  gruvboxDark,
  tokyoNight,
  tokyoNightStorm,
  solarizedDark,
  ayuDark,
  ayuMirage,
  everforestDark,
  rosePine,
  rosePineMoon,
  monokaiPro,
  synthwave84,
  horizonDark,
  palenight,
  oxocarbonDark,
  kanagawa,
  nightfox,
  cyberdream,
  // New gogh themes - Light
  catppuccinLatte,
  gruvboxLight,
  tokyoNightLight,
  solarizedLight,
  ayuLight,
  everforestLight,
  rosePineDawn,
  // Markd themes
  wordLight,
  wordDark,
  githubLight,
  oneLight,
  sepia,
  vitesseLight,
  quietLight,
  githubDark,
  nightOwl,
  vitesseDark,
  cobalt2,
  zenburn,
  moonlight
} from './themeColor'
import { isLinux } from './index'
import { DEFAULT_PAGE_MARGIN_MM, getPageDimensions } from 'common/pageLayout'

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const ORIGINAL_THEME = '#409EFF'

const patchTheme = (css: string): string => {
  return `@media not print {\n${css}\n}`
}

const getEmojiPickerPatch = (): string => {
  return isLinux
    ? '.mu-emoji-picker section .emoji-wrapper .item span { font-family: sans-serif, "Noto Color Emoji"; }'
    : ''
}

/**
 * Raw (not print-guarded) CSS of a built-in editor theme: CSS variables plus
 * the matching Prism code-highlighting theme. Empty for unknown themes; the
 * default light theme only remaps a couple of variables (its values live in
 * the base stylesheet).
 */
export const getThemeCss = (theme: string): string => {
  switch (theme) {
    case 'light':
      return ':root {\n  --link-color: var(--linkColor);\n  --blockquote-border-color: var(--blockquoteBorderColor);\n}'
    case 'dark':
      return dark()
    case 'material-dark':
      return materialDark()
    case 'ulysses':
      return ulysses()
    case 'graphite':
      return graphite()
    case 'one-dark':
      return oneDark()
    // New gogh themes - Dark
    case 'dracula':
      return dracula()
    case 'nord':
      return nord()
    case 'catppuccin-mocha':
      return catppuccinMocha()
    case 'gruvbox-dark':
      return gruvboxDark()
    case 'tokyo-night':
      return tokyoNight()
    case 'tokyo-night-storm':
      return tokyoNightStorm()
    case 'solarized-dark':
      return solarizedDark()
    case 'ayu-dark':
      return ayuDark()
    case 'ayu-mirage':
      return ayuMirage()
    case 'everforest-dark':
      return everforestDark()
    case 'rose-pine':
      return rosePine()
    case 'rose-pine-moon':
      return rosePineMoon()
    case 'monokai-pro':
      return monokaiPro()
    case 'synthwave-84':
      return synthwave84()
    case 'horizon-dark':
      return horizonDark()
    case 'palenight':
      return palenight()
    case 'oxocarbon-dark':
      return oxocarbonDark()
    case 'kanagawa':
      return kanagawa()
    case 'nightfox':
      return nightfox()
    case 'cyberdream':
      return cyberdream()
    // New gogh themes - Light
    case 'catppuccin-latte':
      return catppuccinLatte()
    case 'gruvbox-light':
      return gruvboxLight()
    case 'tokyo-night-light':
      return tokyoNightLight()
    case 'solarized-light':
      return solarizedLight()
    case 'ayu-light':
      return ayuLight()
    case 'everforest-light':
      return everforestLight()
    case 'rose-pine-dawn':
      return rosePineDawn()
    // Markd themes
    case 'word':
      return wordLight()
    case 'word-dark':
      return wordDark()
    case 'github-light':
      return githubLight()
    case 'one-light':
      return oneLight()
    case 'sepia':
      return sepia()
    case 'vitesse-light':
      return vitesseLight()
    case 'quiet-light':
      return quietLight()
    case 'github-dark':
      return githubDark()
    case 'night-owl':
      return nightOwl()
    case 'vitesse-dark':
      return vitesseDark()
    case 'cobalt2':
      return cobalt2()
    case 'zenburn':
      return zenburn()
    case 'moonlight':
      return moonlight()
    default:
      return ''
  }


}

export const addThemeStyle = (theme: string): void => {
  const isCmRailscasts = railscastsThemes.includes(theme)
  const isCmOneDark = oneDarkThemes.includes(theme)
  const isDarkTheme = isCmOneDark || isCmRailscasts
  let themeStyleEle = document.querySelector(`#${THEME_STYLE_ID}`) as HTMLStyleElement | null
  if (!themeStyleEle) {
    themeStyleEle = document.createElement('style')
    themeStyleEle.id = THEME_STYLE_ID
    document.head.appendChild(themeStyleEle)
  }

  themeStyleEle.innerHTML = patchTheme(getThemeCss(theme))

  // workaround: use dark icons
  document.body.classList.remove('dark')
  if (isDarkTheme) {
    document.body.classList.add('dark')
  }

  // change CodeMirror theme
  const cm = document.querySelector('.CodeMirror')
  if (cm) {
    cm.classList.remove('cm-s-default')
    cm.classList.remove('cm-s-one-dark')
    cm.classList.remove('cm-s-railscasts')
    if (isCmOneDark) {
      cm.classList.add('cm-s-one-dark')
    } else if (isCmRailscasts) {
      cm.classList.add('cm-s-railscasts')
    } else {
      cm.classList.add('cm-s-default')
    }
  }
}

export const setEditorWidth = (value: string): void => {
  const EDITOR_WIDTH_STYLE_ID = 'editor-width'
  let result = ''
  if (value && /^[0-9]+(?:ch|px|%)$/.test(value)) {
    // Add 100px for the container's horizontal padding. Set both the legacy
    // camelCase var (source mode) and the kebab-case var the active
    // @muyajs/core engine reads for `.mu-container` max-width (issue #4828).
    const width = `calc(100px + ${value})`
    result = `:root { --editorAreaWidth: ${width}; --editor-area-width: ${width}; }`
  }
  let styleEle = document.querySelector(`#${EDITOR_WIDTH_STYLE_ID}`) as HTMLStyleElement | null
  if (!styleEle) {
    styleEle = document.createElement('style')
    styleEle.setAttribute('id', EDITOR_WIDTH_STYLE_ID)
    document.head.appendChild(styleEle)
  }

  styleEle.innerHTML = result
}

const PAGE_LAYOUT_STYLE_ID = 'page-layout'

/**
 * Draw the WYSIWYG writing column as a sheet of paper (A4 / Letter / Legal)
 * using real millimetre units and page margins, so lines wrap exactly like the
 * exported DOCX/PDF. A faint rule marks every page height as a page-break hint.
 * Passing `'none'` restores the free-width editor.
 */
export const setPageLayout = (layout: string, marginMm?: number): void => {
  let styleEle = document.querySelector(`#${PAGE_LAYOUT_STYLE_ID}`) as HTMLStyleElement | null
  if (!styleEle) {
    styleEle = document.createElement('style')
    styleEle.id = PAGE_LAYOUT_STYLE_ID
    document.head.appendChild(styleEle)
  }

  const page = getPageDimensions(layout)
  document.body.classList.toggle('page-layout', !!page)
  if (!page) {
    styleEle.innerHTML = ''
    return
  }

  const margin =
    typeof marginMm === 'number' && marginMm >= 5 && marginMm <= 51 ? marginMm : DEFAULT_PAGE_MARGIN_MM
  styleEle.innerHTML = `@media not print {
.page-layout .editor-wrapper:not(.source) .editor-component {
  background: var(--pageDeskColor, color-mix(in srgb, var(--editorBgColor) 86%, #808080));
}
.page-layout .editor-wrapper:not(.source) .editor-component .mu-container {
  --editor-area-width: ${page.width}mm;
  width: ${page.width}mm;
  max-width: none;
  min-height: ${page.height}mm;
  /* Bottom room lets the last heading scroll to the top, as in free-width mode. */
  margin: 32px auto 100vh;
  padding: ${margin}mm !important;
  background-color: var(--editorBgColor);
  background-image: repeating-linear-gradient(
    to bottom,
    transparent 0,
    transparent calc(${page.height}mm - 1px),
    var(--editorColor10) calc(${page.height}mm - 1px),
    var(--editorColor10) ${page.height}mm
  );
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.18), 0 6px 24px rgba(0, 0, 0, 0.14);
  border-radius: 2px;
}
}`
}

export interface CommonStyleOptions {
  codeFontFamily: string
  codeFontSize: number | string
  hideScrollbar?: boolean
  [key: string]: unknown
}

export const addCommonStyle = (options: CommonStyleOptions): void => {
  const { codeFontFamily, codeFontSize, hideScrollbar } = options
  let sheet = document.querySelector(`#${COMMON_STYLE_ID}`) as HTMLStyleElement | null
  if (!sheet) {
    sheet = document.createElement('style')
    sheet.id = COMMON_STYLE_ID
    document.head.appendChild(sheet)
  }

  let scrollbarStyle = ''
  if (hideScrollbar) {
    scrollbarStyle = '::-webkit-scrollbar {display: none;}'
  }

  sheet.innerHTML = `${scrollbarStyle}
.CodeMirror {
font-family: ${codeFontFamily}, ${DEFAULT_CODE_FONT_FAMILY};
font-size: ${codeFontSize}px;
}

${getEmojiPickerPatch()}
`
}

export interface CustomStyleOptions {
  customCss?: string
  [key: string]: unknown
}

export const addCustomStyle = (options: CustomStyleOptions): void => {
  const { customCss } = options
  if (!customCss) {
    document.querySelector('#custom-styles')?.remove()
    return
  }

  let customStyleEle = document.querySelector('#custom-styles') as HTMLStyleElement | null
  if (!customStyleEle) {
    customStyleEle = document.createElement('style')
    customStyleEle.id = 'custom-styles'
    document.head.appendChild(customStyleEle)
  }
  customStyleEle.innerHTML = customCss
}

export interface AddStylesOptions extends CommonStyleOptions {
  theme: string
}

// Append common sheet and theme at the end of head - order is important.
export const addStyles = (options: AddStylesOptions): void => {
  const { theme } = options
  addThemeStyle(theme)
  addCommonStyle(options)
}
