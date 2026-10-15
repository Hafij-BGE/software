// Page layout ("paper") mode for the editor. When enabled the writing column
// is drawn as a sheet of paper with the real page width and margins, so text
// wraps exactly as it will in the exported DOCX / PDF.

export type PageLayoutId = 'none' | 'a4' | 'letter' | 'legal'

export interface PageDimensions {
  /** Width in millimetres. */
  width: number
  /** Height in millimetres. */
  height: number
  /** Electron `printToPDF` page size name. */
  printName: 'A4' | 'Letter' | 'Legal'
}

export const PAGE_DIMENSIONS: Readonly<Record<Exclude<PageLayoutId, 'none'>, PageDimensions>> =
  Object.freeze({
    a4: { width: 210, height: 297, printName: 'A4' },
    letter: { width: 215.9, height: 279.4, printName: 'Letter' },
    legal: { width: 215.9, height: 355.6, printName: 'Legal' }
  })

export const DEFAULT_PAGE_MARGIN_MM = 25.4

export const getPageDimensions = (layout: string | undefined): PageDimensions | null =>
  layout && layout in PAGE_DIMENSIONS
    ? PAGE_DIMENSIONS[layout as keyof typeof PAGE_DIMENSIONS]
    : null
