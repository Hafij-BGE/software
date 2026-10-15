// Keyboard shortcuts for a hardware keyboard (a desktop host, or a tablet's
// keyboard): the file commands and find, as in other editors. Formatting keys
// (Ctrl+B, Ctrl+Z…) are muya's own.

import { openDocument, saveDocument, saveDocumentAs } from './document'
import { closeFind, openFind } from './commands'
import { exportPdf } from './export'

export const initKeys = (): void => {
  document.addEventListener('keydown', (event) => {
    if (event.defaultPrevented || event.isComposing) return
    const mod = event.ctrlKey || event.metaKey
    const key = event.key.toLowerCase()
    let run: (() => unknown) | null = null
    if (mod && !event.altKey) {
      if (key === 's') run = event.shiftKey ? saveDocumentAs : saveDocument
      else if (key === 'o' && !event.shiftKey) run = openDocument
      else if (key === 'f' && !event.shiftKey) run = () => openFind(false)
      else if (key === 'h' && !event.shiftKey) run = () => openFind(true)
      else if (key === 'p' && !event.shiftKey) run = exportPdf
    } else if (key === 'escape' && !document.getElementById('find-bar')!.hidden) {
      run = closeFind
    }
    if (!run) return
    event.preventDefault()
    event.stopPropagation()
    run()
  }, true)
}
