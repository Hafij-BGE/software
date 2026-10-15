// Side drawer: new / open, recent files and browsable folders (SAF trees).

import { ICONS } from './icons'
import { callNative, isCancelled, isNative, type TreeEntry } from './native'
import { doc, getRecent, newDocument, openDocument, openUri, removeRecent } from './document'
import { h, promptDialog, pushBack, toast } from './ui'

interface SavedFolder {
  treeUri: string
  name: string
  rootId: string
}

const FOLDERS_KEY = 'markd.folders'
const MARKDOWN_REG = /\.(md|markdown|mdown|mkd|txt)$/i

const getFolders = (): SavedFolder[] => {
  try {
    return JSON.parse(localStorage.getItem(FOLDERS_KEY) || '[]') as SavedFolder[]
  } catch {
    return []
  }
}

const setFolders = (folders: SavedFolder[]): void => {
  try {
    localStorage.setItem(FOLDERS_KEY, JSON.stringify(folders))
  } catch {
    // Not fatal: the folder simply is not remembered.
  }
}

const relativeTime = (time: number): string => {
  const minutes = Math.round((Date.now() - time) / 60000)
  if (minutes < 1) return 'just now'
  if (minutes < 60) return `${minutes} min ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours} h ago`
  return new Date(time).toLocaleDateString()
}

let unregisterBack: (() => void) | null = null
/** Folder being browsed: the saved folder plus the stack of opened sub-folders. */
let browsing: { folder: SavedFolder, path: Array<{ id: string, name: string }> } | null = null

const drawer = (): HTMLElement => document.getElementById('drawer')!

export const closeDrawer = (): void => {
  drawer().classList.remove('open')
  document.getElementById('drawer-scrim')!.classList.remove('open')
  unregisterBack?.()
  unregisterBack = null
}

/** Back steps out of a browsed folder first, then closes the drawer. */
const registerBack = (): void => {
  unregisterBack = pushBack(() => {
    unregisterBack = null
    if (browsing) {
      browsing.path.pop()
      if (!browsing.path.length) browsing = null
      render()
      registerBack()
    } else closeDrawer()
  })
}

export const openDrawer = (): void => {
  render()
  drawer().classList.add('open')
  document.getElementById('drawer-scrim')!.classList.add('open')
  unregisterBack?.()
  registerBack()
}

const action = (label: string, icon: string, run: () => void): HTMLElement =>
  h('button', { type: 'button', class: 'drawer-action', onClick: run, html: `${icon}<span>${label}</span>` })

const addFolder = async(): Promise<void> => {
  try {
    const folder = await callNative<SavedFolder>('openFolder')
    const folders = getFolders().filter((item) => item.treeUri !== folder.treeUri)
    folders.unshift(folder)
    setFolders(folders)
    browsing = { folder, path: [{ id: folder.rootId, name: folder.name }] }
    render()
  } catch (err) {
    if (!isCancelled(err)) toast((err as Error).message)
  }
}

const renderHome = (body: HTMLElement): void => {
  body.append(
    h('div', { class: 'drawer-actions' },
      action('New', ICONS.newFile(22), () => {
        closeDrawer()
        newDocument()
      }),
      action('Open', ICONS.open(22), () => {
        closeDrawer()
        openDocument()
      }),
      isNative() ? action('Open folder', ICONS.folder(22), addFolder) : null
    )
  )

  const recent = getRecent()
  body.append(h('h3', { class: 'drawer-heading' }, 'Recent'))
  if (!recent.length) body.append(h('p', { class: 'drawer-empty' }, 'Files you open appear here.'))
  for (const item of recent) {
    const row = h('div', { class: `drawer-row${item.uri === doc.uri ? ' current' : ''}` },
      h('button', {
        type: 'button',
        class: 'drawer-item',
        onClick: async() => {
          closeDrawer()
          if (item.uri !== doc.uri && !(await openUri(item.uri))) {
            removeRecent(item.uri)
          }
        },
        html: `${ICONS.file(20)}<span class="drawer-item-text"><span class="drawer-item-name"></span><span class="drawer-item-hint">${relativeTime(item.time)}</span></span>`
      }),
      h('button', {
        type: 'button',
        class: 'icon-btn small',
        'aria-label': `Remove ${item.name} from recent files`,
        html: ICONS.close,
        onClick: () => {
          removeRecent(item.uri)
          render()
        }
      })
    )
    row.querySelector('.drawer-item-name')!.textContent = item.name
    body.append(row)
  }

  const folders = getFolders()
  if (folders.length) {
    body.append(h('h3', { class: 'drawer-heading' }, 'Folders'))
    for (const folder of folders) {
      const row = h('div', { class: 'drawer-row' },
        h('button', {
          type: 'button',
          class: 'drawer-item',
          onClick: () => {
            browsing = { folder, path: [{ id: folder.rootId, name: folder.name }] }
            render()
          },
          html: `${ICONS.folder(20)}<span class="drawer-item-text"><span class="drawer-item-name"></span></span>`
        }),
        h('button', {
          type: 'button',
          class: 'icon-btn small',
          'aria-label': `Forget folder ${folder.name}`,
          html: ICONS.close,
          onClick: () => {
            setFolders(getFolders().filter((item) => item.treeUri !== folder.treeUri))
            callNative('releaseFolder', { treeUri: folder.treeUri }).catch(() => {})
            render()
          }
        })
      )
      row.querySelector('.drawer-item-name')!.textContent = folder.name
      body.append(row)
    }
  }
}

const renderFolder = async(body: HTMLElement): Promise<void> => {
  const state = browsing!
  const current = state.path[state.path.length - 1]!
  body.append(
    h('div', { class: 'drawer-crumbs' },
      h('button', {
        type: 'button',
        class: 'icon-btn small',
        'aria-label': 'Up',
        html: ICONS.up(20),
        onClick: () => {
          state.path.pop()
          if (!state.path.length) browsing = null
          render()
        }
      }),
      h('span', { class: 'drawer-crumb-text' }, state.path.map((p) => p.name).join(' / ')),
      h('button', {
        type: 'button',
        class: 'btn small',
        onClick: async() => {
          const name = await promptDialog('New document', 'File name', 'Untitled.md')
          if (!name) return
          try {
            const created = await callNative<{ uri: string }>('createInFolder', {
              treeUri: state.folder.treeUri,
              docId: current.id,
              name: MARKDOWN_REG.test(name) ? name : `${name}.md`
            })
            closeDrawer()
            await openUri(created.uri)
          } catch (err) {
            toast((err as Error).message)
          }
        }
      }, '+ New')
    )
  )
  const list = h('div', { class: 'drawer-list' }, h('p', { class: 'drawer-empty' }, 'Loading…'))
  body.append(list)
  try {
    const entries = await callNative<TreeEntry[]>('listFolder', { treeUri: state.folder.treeUri, docId: current.id })
    const visible = entries
      .filter((entry) => !entry.name.startsWith('.') && (entry.isDir || MARKDOWN_REG.test(entry.name)))
      .sort((a, b) => Number(b.isDir) - Number(a.isDir) || a.name.localeCompare(b.name))
    list.replaceChildren()
    if (!visible.length) list.append(h('p', { class: 'drawer-empty' }, 'No Markdown files here.'))
    for (const entry of visible) {
      const button = h('button', {
        type: 'button',
        class: `drawer-item${entry.uri === doc.uri ? ' current' : ''}`,
        html: `${entry.isDir ? ICONS.folder(20) : ICONS.file(20)}<span class="drawer-item-text"><span class="drawer-item-name"></span>${entry.isDir ? '' : `<span class="drawer-item-hint">${relativeTime(entry.modified)}</span>`}</span>`,
        onClick: async() => {
          if (entry.isDir) {
            state.path.push({ id: entry.docId, name: entry.name })
            render()
            return
          }
          closeDrawer()
          await openUri(entry.uri)
        }
      })
      button.querySelector('.drawer-item-name')!.textContent = entry.name
      list.append(h('div', { class: 'drawer-row' }, button))
    }
  } catch (err) {
    list.replaceChildren(h('p', { class: 'drawer-empty' }, `Cannot read folder: ${(err as Error).message}`))
  }
}

const render = (): void => {
  const body = document.getElementById('drawer-body')!
  body.replaceChildren()
  if (browsing) renderFolder(body)
  else renderHome(body)
}

export const initDrawer = (): void => {
  document.getElementById('drawer-scrim')!.addEventListener('click', closeDrawer)
  document.getElementById('drawer-close')!.addEventListener('click', closeDrawer)
}
