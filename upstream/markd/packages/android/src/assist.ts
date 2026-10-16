// What a host's assistant reads and changes (Craft Suite's Ask AI), through
// native events: the Markdown and the selected text, and a few edits. Edits go
// through the editor's own content, so Undo takes them back.

import { currentMarkdown, getMuya, insertMarkdownAtCursor, notifyChange, rememberCursor } from './editor'

export interface Snapshot {
  markdown: string
  selection: string
}

export interface EditResult {
  ok: boolean
  error?: string
}

/** The document now (and remember the caret, where inserted text goes). */
export const snapshot = (): Snapshot => {
  rememberCursor()
  return { markdown: currentMarkdown(), selection: String(window.getSelection() ?? '') }
}

/** The whole document, as one step Undo takes back. */
const replaceContent = (markdown: string): void => {
  getMuya().replaceContent(markdown)
  notifyChange()
}

const text = (params: Record<string, unknown>, name: string): string => {
  const value = params[name]
  if (typeof value !== 'string') throw new Error(`"${name}" must be text`)
  return value
}

/** One edit: `text.insert`, `text.append`, `edit.replaceAll` or `document.replace`. */
export const applyEdit = (payload: unknown): EditResult => {
  const { command, params = {} } = (payload ?? {}) as { command?: string, params?: Record<string, unknown> }
  try {
    switch (command) {
      case 'text.insert':
        insertMarkdownAtCursor(text(params, 'text'))
        break
      case 'text.append': {
        const md = currentMarkdown().replace(/\s*$/, '')
        replaceContent(`${md}${md ? '\n\n' : ''}${text(params, 'text')}\n`)
        break
      }
      case 'edit.replaceAll': {
        const find = text(params, 'text')
        if (!find) throw new Error('nothing to find')
        const before = currentMarkdown()
        if (!before.includes(find)) throw new Error(`"${find}" isn't in the document`)
        replaceContent(before.split(find).join(text(params, 'with')))
        break
      }
      case 'document.replace':
        replaceContent(text(params, 'markdown'))
        break
      default:
        throw new Error(`unknown edit ${String(command)}`)
    }
    return { ok: true }
  } catch (err) {
    return { ok: false, error: (err as Error).message }
  }
}
