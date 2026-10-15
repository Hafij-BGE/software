import bus from '@/bus'
import { usePreferencesStore } from '@/store/preferences'

/**
 * Export without the options dialog: the document is exported with the theme
 * it is being edited in, and with the editor's page layout (size, margins and
 * font) so the result wraps like the screen. Free-width documents fall back to
 * A4 with 25.4 mm margins.
 */
export const quickExport = (type: 'pdf' | 'docx' | 'styledHtml' | 'print'): void => {
  const { editorFontFamily, fontSize, lineHeight, pageMargin } = usePreferencesStore()
  const margin = typeof pageMargin === 'number' ? pageMargin : 25.4
  bus.emit('editor-blur')
  bus.emit('export', {
    type,
    theme: 'editor',
    matchEditorLayout: true,
    pageSize: 'A4',
    isLandscape: false,
    pageMarginTop: margin,
    pageMarginRight: margin,
    pageMarginBottom: margin,
    pageMarginLeft: margin,
    fontFamily: editorFontFamily || null,
    fontSize,
    lineHeight,
    autoNumberingHeadings: false,
    showFrontMatter: false,
    tocTitle: '',
    tocIncludeTopHeading: true,
    htmlTitle: ''
  })
}
