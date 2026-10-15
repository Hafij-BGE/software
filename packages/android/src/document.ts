// The open document: Storage Access Framework open/save through the native
// bridge, the recent-files list, and saving images next to the .md file.
// Outside the APK (vite dev server in a desktop browser) it falls back to a
// file input and downloads, which is enough to work on the UI.

import { blobToBase64, blobToDataUrl, callNative, isCancelled, isNative, type DocumentInfo } from './native'
import { currentMarkdown, getMuya, loadDocument } from './editor'
import { settings } from './settings'
import { choiceDialog, confirmDialog, toast } from './ui'

export interface RecentFile {
  uri: string
  name: string
  time: number
}

interface CurrentDoc {
  uri: string | null
  name: string
  hasFolder: boolean
  savedMarkdown: string
}

export const doc: CurrentDoc = { uri: null, name: 'Untitled.md', hasFolder: false, savedMarkdown: '' }

const RECENT_KEY = 'markd.recent'
const DRAFT_KEY = 'markd.draft'
const LAST_KEY = 'markd.last'
const MAX_RECENT = 20

type DocListener = () => void
const listeners = new Set<DocListener>()
export const onDocChange = (listener: DocListener): void => {
  listeners.add(listener)
}
const notify = (): void => {
  for (const listener of listeners) listener()
}

const readJson = <T>(key: string, fallback: T): T => {
  try {
    const raw = localStorage.getItem(key)
    return raw ? (JSON.parse(raw) as T) : fallback
  } catch {
    return fallback
  }
}

const writeJson = (key: string, value: unknown): void => {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // Quota / private mode: recents and drafts are a convenience only.
  }
}

export const getRecent = (): RecentFile[] => readJson<RecentFile[]>(RECENT_KEY, [])

const addRecent = (uri: string, name: string): void => {
  const list = getRecent().filter((item) => item.uri !== uri)
  list.unshift({ uri, name, time: Date.now() })
  writeJson(RECENT_KEY, list.slice(0, MAX_RECENT))
}

export const removeRecent = (uri: string): void => {
  writeJson(RECENT_KEY, getRecent().filter((item) => item.uri !== uri))
}

export const isDirty = (): boolean => {
  try {
    return currentMarkdown() !== doc.savedMarkdown
  } catch {
    return false
  }
}

const setCurrent = (info: { uri: string | null, name: string, hasFolder: boolean }, markdown: string): void => {
  doc.uri = info.uri
  doc.name = info.name
  doc.hasFolder = info.hasFolder
  doc.savedMarkdown = markdown
  if (info.uri) {
    addRecent(info.uri, info.name)
    writeJson(LAST_KEY, info.uri)
  } else {
    writeJson(LAST_KEY, null)
  }
  notify()
}

const confirmDiscard = async(): Promise<boolean> => {
  if (!isDirty()) return true
  if (doc.uri && settings.autoSave) {
    return saveDocument()
  }
  const choice = await choiceDialog('Unsaved changes', [
    { value: 'save', label: 'Save', hint: doc.uri ? doc.name : 'Choose where to save it' },
    { value: 'discard', label: 'Discard changes' }
  ])
  if (choice === 'save') return saveDocument()
  return choice === 'discard'
}

/** Native serves /doc/ images from this document's folder, so tell it before rendering. */
const announceDocument = async(uri: string | null): Promise<void> => {
  if (isNative()) await callNative('setCurrentDocument', { uri: uri ?? '' }).catch(() => {})
}

const openInfo = async(info: DocumentInfo): Promise<void> => {
  const text = info.text.replace(/\r\n?/g, '\n')
  await announceDocument(info.uri)
  loadDocument(text)
  // Muya normalises the source on load; compare dirtiness against that.
  setCurrent(info, currentMarkdown())
  localStorage.removeItem(DRAFT_KEY)
}

export const newDocument = async(): Promise<void> => {
  if (!(await confirmDiscard())) return
  await announceDocument(null)
  loadDocument('')
  setCurrent({ uri: null, name: 'Untitled.md', hasFolder: false }, currentMarkdown())
}

const pickFileInBrowser = (accept: string): Promise<File | null> =>
  new Promise((resolve) => {
    const input = document.createElement('input')
    input.type = 'file'
    input.accept = accept
    input.onchange = () => resolve(input.files?.[0] ?? null)
    input.click()
  })

export const openDocument = async(): Promise<void> => {
  if (!(await confirmDiscard())) return
  try {
    if (isNative()) {
      await openInfo(await callNative<DocumentInfo>('openDocument'))
    } else {
      const file = await pickFileInBrowser('.md,.markdown,.mdown,.txt,text/markdown,text/plain')
      if (file) await openInfo({ uri: null as unknown as string, name: file.name, text: await file.text(), hasFolder: false })
    }
  } catch (err) {
    if (!isCancelled(err)) toast(`Could not open: ${(err as Error).message}`)
  }
}

export const openUri = async(uri: string, { askDiscard = true } = {}): Promise<boolean> => {
  if (askDiscard && !(await confirmDiscard())) return false
  try {
    await openInfo(await callNative<DocumentInfo>('readDocument', { uri }))
    return true
  } catch (err) {
    toast(`Could not open: ${(err as Error).message}`)
    return false
  }
}

const downloadBlob = (blob: Blob, name: string): void => {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = name
  a.click()
  setTimeout(() => URL.revokeObjectURL(url), 30_000)
}

export const saveDocumentAs = async(): Promise<boolean> => {
  const markdown = currentMarkdown()
  if (!isNative()) {
    downloadBlob(new Blob([markdown], { type: 'text/markdown' }), doc.name)
    doc.savedMarkdown = markdown
    notify()
    return true
  }
  try {
    const info = await callNative<{ uri: string, name: string, hasFolder: boolean }>('createDocument', {
      name: doc.name.match(/\.(md|markdown|mdown|txt)$/i) ? doc.name : `${doc.name}.md`,
      mime: 'text/markdown'
    })
    await callNative('writeDocument', { uri: info.uri, text: markdown })
    await announceDocument(info.uri)
    setCurrent(info, markdown)
    localStorage.removeItem(DRAFT_KEY)
    toast(`Saved ${info.name}`)
    return true
  } catch (err) {
    if (!isCancelled(err)) toast(`Could not save: ${(err as Error).message}`)
    return false
  }
}

export const saveDocument = async(): Promise<boolean> => {
  if (!doc.uri) return saveDocumentAs()
  const markdown = currentMarkdown()
  try {
    await callNative('writeDocument', { uri: doc.uri, text: markdown })
    doc.savedMarkdown = markdown
    notify()
    return true
  } catch (err) {
    toast(`Could not save: ${(err as Error).message}`)
    return false
  }
}

let autoSaveTimer = 0
/** Debounced save for documents that already live in a file; drafts go to storage. */
export const scheduleAutoSave = (): void => {
  window.clearTimeout(autoSaveTimer)
  autoSaveTimer = window.setTimeout(() => {
    if (!isDirty()) return
    if (doc.uri && settings.autoSave) saveDocument()
    else if (!doc.uri) writeJson(DRAFT_KEY, currentMarkdown())
  }, 1500)
}

/** Called when the app goes to the background. */
export const flushOnPause = async(): Promise<void> => {
  window.clearTimeout(autoSaveTimer)
  if (!isDirty()) return
  if (doc.uri && settings.autoSave) await saveDocument()
  else if (!doc.uri) writeJson(DRAFT_KEY, currentMarkdown())
}

/** Reopen what the user had open last time, or the unsaved draft. */
export const restoreSession = async(launchUri: string | null): Promise<void> => {
  if (launchUri && (await openUri(launchUri, { askDiscard: false }))) return
  const last = readJson<string | null>(LAST_KEY, null)
  if (last && isNative() && (await openUri(last, { askDiscard: false }))) return
  const draft = readJson<string | null>(DRAFT_KEY, null)
  if (draft) {
    loadDocument(draft)
    setCurrent({ uri: null, name: 'Untitled.md', hasFolder: false }, '')
  } else {
    setCurrent({ uri: null, name: 'Untitled.md', hasFolder: false }, currentMarkdown())
  }
}

/** Make sure the document's folder is writable so images can sit next to it. */
const ensureFolderAccess = async(): Promise<boolean> => {
  if (doc.hasFolder) return true
  if (!isNative()) return false
  if (!doc.uri) {
    const save = await confirmDialog(
      'Save the document first',
      'Images are stored in an "assets" folder next to the .md file. Save the document now? Otherwise the image is embedded in the document.',
      'Save'
    )
    if (!save || !(await saveDocumentAs())) return false
    if (doc.hasFolder) return true
  }
  const grant = await confirmDialog(
    'Allow access to the folder',
    `To save images next to ${doc.name}, pick the folder that contains it. Otherwise the image is embedded in the document.`,
    'Choose folder'
  )
  if (!grant) return false
  try {
    const result = await callNative<{ hasFolder: boolean }>('grantFolderAccess', { uri: doc.uri })
    doc.hasFolder = result.hasFolder
    if (!doc.hasFolder) toast('That folder does not contain this document')
    notify()
    return doc.hasFolder
  } catch (err) {
    if (!isCancelled(err)) toast((err as Error).message)
    return false
  }
}

/** The page referenced a relative image the app cannot reach yet. */
export const offerFolderAccess = async(): Promise<void> => {
  if (doc.hasFolder || !doc.uri || !isNative()) return
  const grant = await confirmDialog(
    'Show images from the folder?',
    `${doc.name} links to images stored next to it. Pick the folder that contains the document to show them.`,
    'Choose folder'
  )
  if (!grant) return
  try {
    const result = await callNative<{ hasFolder: boolean }>('grantFolderAccess', { uri: doc.uri })
    doc.hasFolder = result.hasFolder
    if (!doc.hasFolder) toast('That folder does not contain this document')
    else {
      // Re-render so the images load from the newly granted folder.
      const markdown = currentMarkdown()
      loadDocument(markdown)
      doc.savedMarkdown = currentMarkdown()
    }
    notify()
  } catch (err) {
    if (!isCancelled(err)) toast((err as Error).message)
  }
}

const timestamp = (): string => {
  const d = new Date()
  const pad = (n: number): string => String(n).padStart(2, '0')
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}${pad(d.getSeconds())}`
}

const EXT_BY_MIME: Record<string, string> = {
  'image/png': 'png',
  'image/jpeg': 'jpg',
  'image/gif': 'gif',
  'image/webp': 'webp',
  'image/svg+xml': 'svg'
}

/**
 * Store an image as `assets/<prefix>-<time>.<ext>` beside the document and
 * return the relative src, or fall back to an embedded data URL.
 */
export const storeImageBlob = async(blob: Blob, prefix: string): Promise<string> => {
  const ext = EXT_BY_MIME[blob.type] ?? 'png'
  if (await ensureFolderAccess()) {
    try {
      const result = await callNative<{ path: string }>('saveAsset', {
        docUri: doc.uri,
        path: `assets/${prefix}-${timestamp()}.${ext}`,
        mime: blob.type || 'image/png',
        base64: await blobToBase64(blob)
      })
      return result.path
    } catch (err) {
      toast(`Image embedded instead: ${(err as Error).message}`)
    }
  }
  return blobToDataUrl(blob)
}

export const pickImage = async(): Promise<string> => {
  try {
    let blob: Blob | null = null
    if (isNative()) {
      const picked = await callNative<{ mime: string, base64: string }>('pickImage')
      blob = await (await fetch(`data:${picked.mime};base64,${picked.base64}`)).blob()
    } else {
      blob = await pickFileInBrowser('image/*')
    }
    return blob ? storeImageBlob(blob, 'image') : ''
  } catch (err) {
    if (!isCancelled(err)) toast(`Could not insert image: ${(err as Error).message}`)
    return ''
  }
}

/** Muya `imageAction`: persist pasted bitmaps; keep URLs and paths as they are. */
export const storeImage = async(src: string): Promise<string> => {
  if (!src.startsWith('data:')) return src
  const blob = await (await fetch(src)).blob()
  return storeImageBlob(blob, 'pasted')
}

/** Save export output (PDF is handled by the print service). */
export const saveExport = async(blob: Blob, name: string): Promise<void> => {
  if (!isNative()) {
    downloadBlob(blob, name)
    return
  }
  try {
    const result = await callNative<{ name: string }>('saveFileAs', {
      name,
      mime: blob.type,
      base64: await blobToBase64(blob)
    })
    toast(`Exported ${result.name}`)
  } catch (err) {
    if (!isCancelled(err)) toast(`Export failed: ${(err as Error).message}`)
  }
}

export const baseName = (): string => doc.name.replace(/\.(md|markdown|mdown|txt)$/i, '') || 'Untitled'
