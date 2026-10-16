import './styles/app.css'
import { wordCount } from '@muyajs/core'
import { ICONS } from './icons'
import { currentMarkdown, getMuya, initEditor } from './editor'
import {
  confirmDiscard,
  doc,
  flushOnPause,
  isDirty,
  offerFolderAccess,
  onDocChange,
  openUri,
  pickImage,
  restoreSession,
  saveDocument,
  scheduleAutoSave,
  storeImage
} from './document'
import { applyHandwriting, initFindBar, initSourceView, isSourceMode, redo, undo } from './commands'
import { closeDrawer, initDrawer, openDrawer } from './drawer'
import { callNative, isNative, onNativeEvent } from './native'
import { applyEdit, snapshot } from './assist'
import { initKeys } from './keys'
import { initPenStatus } from './pen'
import { initRibbon, refreshRibbon } from './ribbon'
import { onSettingsChange, settings, updateSettings } from './settings'
import { applyZoom, currentZoom, getPageDimensions, initTheme, MM_TO_PX } from './theme'
import { handleBack } from './ui'

const $ = (id: string): HTMLElement => document.getElementById(id)!

const updateTitle = (): void => {
  $('doc-name').textContent = doc.name
  $('dirty-dot').hidden = !isDirty()
  document.title = `${doc.name} - Markd`
}

let statusTimer = 0
const updateStatus = (): void => {
  window.clearTimeout(statusTimer)
  statusTimer = window.setTimeout(() => {
    const markdown = currentMarkdown()
    const count = wordCount(markdown)
    $('status-words').textContent = `${count.word.toLocaleString()} words`
    const page = getPageDimensions(settings.pageLayout)
    const sheet = $('page')
    if (page && !isSourceMode()) {
      // Tolerance: the sheet's min-height rounds up to a whole pixel.
      const pages = Math.max(1, Math.ceil(sheet.scrollHeight / (page.height * MM_TO_PX) - 0.01))
      $('status-pages').textContent = `${pages} ${pages === 1 ? 'page' : 'pages'}`
    } else {
      $('status-pages').textContent = isSourceMode() ? 'Source' : 'Mobile view'
    }
    $('status-zoom').textContent = `${Math.round(currentZoom() * 100)}%`
  }, 250)
}

const wireTitleBar = (): void => {
  $('btn-menu').innerHTML = ICONS.menu
  $('btn-undo').innerHTML = ICONS.undo
  $('btn-redo').innerHTML = ICONS.redo
  $('btn-save').innerHTML = ICONS.save
  $('btn-menu').addEventListener('click', openDrawer)
  $('btn-undo').addEventListener('click', undo)
  $('btn-redo').addEventListener('click', redo)
  $('btn-save').addEventListener('click', () => saveDocument())
  for (const id of ['btn-undo', 'btn-redo', 'btn-save']) {
    $(id).addEventListener('mousedown', (event) => event.preventDefault())
  }
  $('status-zoom').addEventListener('click', () => {
    updateSettings({ zoom: settings.zoom === 0 ? 1 : 0 })
  })
}

const wireNativeEvents = (): void => {
  onNativeEvent('pause', () => {
    flushOnPause()
  })
  // Return value tells MainActivity whether the back press was consumed.
  onNativeEvent('back', () => {
    if (handleBack()) return true
    closeDrawer()
    return false
  })
  onNativeEvent('folderNeeded', () => {
    offerFolderAccess()
  })
  onNativeEvent('open', (payload) => {
    const { uri } = payload as { uri: string }
    openUri(uri)
  })
  // A host closing the document (a desktop tab): resolves true once it is saved or discarded.
  onNativeEvent('close', () => confirmDiscard())
  onNativeEvent('save', () => saveDocument())
  // A host's assistant: what the document says, and its edits.
  onNativeEvent('snapshot', () => snapshot())
  onNativeEvent('edit', (payload) => applyEdit(payload))
}

const boot = async(): Promise<void> => {
  initTheme()
  wireTitleBar()
  initEditor('', {
    pickImage,
    storeImage,
    onChange: () => {
      updateTitle()
      updateStatus()
      scheduleAutoSave()
    },
    onSelectionChange: () => {}
  })
  initRibbon()
  initDrawer()
  initFindBar()
  initSourceView()
  initPenStatus()
  initKeys()
  wireNativeEvents()
  onDocChange(() => {
    updateTitle()
    updateStatus()
  })
  onSettingsChange((patch) => {
    if ('pageLayout' in patch || 'pageMargin' in patch || 'zoom' in patch || 'theme' in patch) {
      refreshRibbon()
      updateStatus()
    }
  })
  document.addEventListener('markd:zoom', updateStatus)
  document.addEventListener('markd:view', updateStatus)
  new ResizeObserver(() => applyZoom()).observe($('desk'))

  let launchUri: string | null = null
  if (isNative()) {
    applyHandwriting()
    launchUri = (await callNative<{ launchUri: string | null }>('getLaunchDocument')).launchUri
  }
  await restoreSession(launchUri)
  updateTitle()
  updateStatus()
  document.body.classList.add('ready')
  if (import.meta.env.DEV) (window as unknown as { muya: unknown }).muya = getMuya()
}

boot()
