// Editor commands shared by the ribbon, the title bar and the back button.

import { currentMarkdown, getMuya, insertImageAtCursor, notifyChange, rememberCursor, setEditorOptions } from './editor'
import { pickImage, storeImageBlob } from './document'
import { openSketch } from './ink'
import { drawInInkwell, inkwellAvailable } from './inkwell'
import { callNative, isNative, type HostInfo } from './native'
import { settings, updateSettings } from './settings'
import { h, pushBack, showDialog, toast } from './ui'

export const isSourceMode = (): boolean => document.body.classList.contains('source-mode')

const guard = (run: () => void) => (): void => {
  if (isSourceMode()) {
    toast('Switch off Source view to use formatting tools')
    return
  }
  run()
}

export const format = (type: string) => guard(() => getMuya().format(type))
export const paragraph = (type: string) => guard(() => getMuya().updateParagraph(type))

export const undo = (): void => {
  if (isSourceMode()) document.execCommand('undo')
  else getMuya().undo()
}

export const redo = (): void => {
  if (isSourceMode()) document.execCommand('redo')
  else getMuya().redo()
}

export const insertLink = format('link')

export const insertTable = guard(async() => {
  rememberCursor()
  const rows = h('input', { type: 'number', class: 'field', min: '2', max: '50', value: '4', inputmode: 'numeric' })
  const columns = h('input', { type: 'number', class: 'field', min: '1', max: '20', value: '3', inputmode: 'numeric' })
  const ok = await showDialog({
    title: 'Insert table',
    body: h('div', { class: 'field-row' },
      h('label', { class: 'field-label' }, 'Rows', rows),
      h('label', { class: 'field-label' }, 'Columns', columns)
    ),
    dismissValue: false,
    buttons: [{ label: 'Cancel', value: false }, { label: 'Insert', value: true, primary: true }]
  })
  if (ok) getMuya().createTable({ rows: Number(rows.value) || 4, columns: Number(columns.value) || 3 })
})

export const insertPicture = guard(async() => {
  rememberCursor()
  const src = await pickImage()
  if (src) insertImageAtCursor(src, 'image')
})

export const insertSketch = guard(async() => {
  rememberCursor()
  getMuya().blur()
  let blob: Blob | null = null
  if (await inkwellAvailable()) {
    try {
      blob = await drawInInkwell()
      if (!blob) return
    } catch (err) {
      toast(`Inkwell: ${(err as Error).message}. Using the built-in pad.`)
    }
  }
  if (!blob) {
    const result = await openSketch()
    if (!result) return
    blob = result.blob
  }
  const src = await storeImageBlob(blob, 'sketch')
  insertImageAtCursor(src, 'Sketch')
  toast('Sketch inserted')
})

/** The simple built-in pad (Draw → Quick sketch). */
export const insertQuickSketch = guard(async() => {
  rememberCursor()
  getMuya().blur()
  const result = await openSketch()
  if (!result) return
  const src = await storeImageBlob(result.blob, 'sketch')
  insertImageAtCursor(src, 'Sketch')
  toast('Sketch inserted')
})

export const selectAll = (): void => {
  if (isSourceMode()) (document.getElementById('source') as HTMLTextAreaElement).select()
  else getMuya().selectAll()
}

// ---- Find & replace ----

let closeFindBack: (() => void) | null = null

export const openFind = (withReplace = false): void => {
  const bar = document.getElementById('find-bar')!
  bar.hidden = false
  bar.classList.toggle('with-replace', withReplace)
  const input = document.getElementById('find-input') as HTMLInputElement
  input.focus()
  input.select()
  closeFindBack?.()
  closeFindBack = pushBack(() => {
    closeFindBack = null
    closeFind()
  })
}

export const closeFind = (): void => {
  document.getElementById('find-bar')!.hidden = true
  closeFindBack?.()
  closeFindBack = null
  try {
    getMuya().search('')
  } catch {
    // Editor not ready.
  }
}

const updateMatchCount = (result: unknown): void => {
  const count = document.getElementById('find-count')!
  const matches = (result as { matches?: unknown[], index?: number } | undefined)
  if (matches?.matches) {
    count.textContent = matches.matches.length ? `${(matches.index ?? 0) + 1}/${matches.matches.length}` : '0/0'
  } else count.textContent = ''
}

export const initFindBar = (): void => {
  const input = document.getElementById('find-input') as HTMLInputElement
  const replaceInput = document.getElementById('replace-input') as HTMLInputElement
  input.addEventListener('input', () => updateMatchCount(getMuya().search(input.value, { isCaseSensitive: false })))
  input.addEventListener('keydown', (event) => {
    if (event.key === 'Enter') updateMatchCount(getMuya().find(event.shiftKey ? 'previous' : 'next'))
  })
  document.getElementById('find-prev')!.addEventListener('click', () => updateMatchCount(getMuya().find('previous')))
  document.getElementById('find-next')!.addEventListener('click', () => updateMatchCount(getMuya().find('next')))
  document.getElementById('find-close')!.addEventListener('click', closeFind)
  document.getElementById('replace-one')!.addEventListener('click', () => {
    updateMatchCount(getMuya().replace(replaceInput.value, { isSingle: true, isRegexp: false }))
  })
  document.getElementById('replace-all')!.addEventListener('click', () => {
    updateMatchCount(getMuya().replace(replaceInput.value, { isSingle: false, isRegexp: false }))
  })
  for (const button of document.querySelectorAll('#find-bar button')) {
    button.addEventListener('mousedown', (event) => event.preventDefault())
  }
}

// ---- Source view ----

let sourceTimer = 0

export const toggleSource = (): void => {
  const source = document.getElementById('source') as HTMLTextAreaElement
  const on = !isSourceMode()
  if (on) {
    source.value = currentMarkdown()
    document.body.classList.add('source-mode')
    source.focus()
  } else {
    window.clearTimeout(sourceTimer)
    getMuya().setContent(source.value)
    document.body.classList.remove('source-mode')
  }
  document.dispatchEvent(new Event('markd:view'))
}

export const initSourceView = (): void => {
  const source = document.getElementById('source') as HTMLTextAreaElement
  source.addEventListener('input', () => {
    window.clearTimeout(sourceTimer)
    sourceTimer = window.setTimeout(() => {
      getMuya().setContent(source.value)
      notifyChange()
    }, 500)
  })
}

export const toggleFocusMode = (): void => {
  updateSettings({ focusMode: !settings.focusMode })
  getMuya().setFocusMode(settings.focusMode)
}

export const setFontSize = (delta: number): void => {
  const fontSize = Math.min(28, Math.max(10, settings.fontSize + delta))
  updateSettings({ fontSize })
  setEditorOptions({ fontSize })
}

// ---- Stylus handwriting ----

export const applyHandwriting = (): void => {
  if (isNative()) callNative('setHandwriting', { enabled: settings.penHandwriting }).catch(() => {})
}

export const toggleHandwriting = (): void => {
  updateSettings({ penHandwriting: !settings.penHandwriting })
  applyHandwriting()
  toast(settings.penHandwriting ? 'Pen handwriting on' : 'Pen handwriting off')
}

export const showHandwritingHelp = async(): Promise<void> => {
  let support = 'Open Markd on your Android device to use stylus handwriting.'
  if (isNative()) {
    const info = await callNative<HostInfo>('getInfo')
    support = info.handwritingSupported
      ? `Android ${info.release} supports stylus handwriting.`
      : `Android ${info.release} does not support stylus handwriting (Android 14 or newer is required). You can still draw sketches.`
  }
  await showDialog({
    title: 'Write with your pen',
    body: h('div', { class: 'help' },
      h('p', {}, support),
      h('ul', {},
        h('li', {}, 'Handwriting: write directly on the page with the stylus. Android turns it into text at that spot. Scribble over words to delete them; draw a vertical line to split or join words.'),
        h('li', {}, 'This needs a keyboard with stylus support (Gboard, Samsung Keyboard) and "Stylus handwriting" turned on in Android settings.'),
        h('li', {}, 'Sketch (Draw tab): pressure-sensitive ink. Tilt the pen for broader strokes, hover to preview the tip, and use the eraser end or the side button to erase strokes. Your palm is ignored while the pen is in use.')
      )
    ),
    dismissValue: null,
    buttons: [{ label: 'OK', value: null, primary: true }]
  })
}

export const showAbout = async(): Promise<void> => {
  let version = ''
  if (isNative()) {
    version = (await callNative<HostInfo>('getInfo')).version
  }
  await showDialog({
    title: 'About Markd',
    body: h('div', { class: 'help' },
      h('p', {}, version ? `Markd ${version}` : 'Markd'),
      h('p', {}, 'Developed by Md. Hafijur Rahman'),
      h('p', {}, 'GitHub: https://github.com/Hafij-BGE/markd'),
      h('p', {}, 'License: MIT. Based on MarkText, Copyright © Luo Ran and GitHub contributors.'),
      h('p', {}, 'This app was generated with AI. Developer support may not be available.')
    ),
    dismissValue: null,
    buttons: [{ label: 'OK', value: null, primary: true }]
  })
}
