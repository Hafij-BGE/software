// Small touch-friendly dialog / toast helpers. Every dialog registers itself as
// the topmost "back" target, so the Android back button closes it first.

const backStack: Array<() => void> = []

export const pushBack = (close: () => void): (() => void) => {
  backStack.push(close)
  return () => {
    const index = backStack.lastIndexOf(close)
    if (index >= 0) backStack.splice(index, 1)
  }
}

/** Close the topmost overlay; `false` when there was nothing to close. */
export const handleBack = (): boolean => {
  const close = backStack.pop()
  if (!close) return false
  close()
  return true
}

let toastTimer = 0
export const toast = (message: string, ms = 2600): void => {
  const el = document.getElementById('toast')!
  el.textContent = message
  el.hidden = false
  window.clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => {
    el.hidden = true
  }, ms)
}

export const h = <K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Record<string, unknown> = {},
  ...children: Array<Node | string | null | undefined | false>
): HTMLElementTagNameMap[K] => {
  const el = document.createElement(tag)
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === null || value === false) continue
    if (key === 'class') el.className = String(value)
    else if (key === 'html') el.innerHTML = String(value)
    else if (key.startsWith('on') && typeof value === 'function') {
      el.addEventListener(key.slice(2).toLowerCase(), value as EventListener)
    } else if (key in el && typeof value !== 'string') {
      (el as unknown as Record<string, unknown>)[key] = value
    } else el.setAttribute(key, String(value))
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue
    el.append(child)
  }
  return el
}

interface DialogButton<T> {
  label: string
  value: T
  primary?: boolean
  danger?: boolean
}

interface DialogOptions<T> {
  title: string
  body?: Node | string
  buttons: Array<DialogButton<T>>
  /** Value returned on back / scrim tap. */
  dismissValue: T
  /** Read form state just before closing with a non-dismiss button. */
  beforeClose?: (value: T) => T
  onOpen?: (dialog: HTMLElement, close: (value: T) => void) => void
}

export const showDialog = <T>(options: DialogOptions<T>): Promise<T> =>
  new Promise((resolve) => {
    const root = document.getElementById('dialog-root')!
    let unregister = (): void => {}
    const close = (value: T): void => {
      unregister()
      scrim.remove()
      resolve(value)
    }
    const dialog = h('div', { class: 'dialog', role: 'dialog', 'aria-modal': 'true' },
      h('h2', { class: 'dialog-title' }, options.title),
      options.body ? h('div', { class: 'dialog-body' }, options.body) : null,
      h('div', { class: 'dialog-actions' },
        ...options.buttons.map((button) =>
          h('button', {
            type: 'button',
            class: `btn${button.primary ? ' primary' : ''}${button.danger ? ' danger' : ''}`,
            onClick: () => close(options.beforeClose && button.value !== options.dismissValue ? options.beforeClose(button.value) : button.value)
          }, button.label)
        )
      )
    )
    const scrim = h('div', { class: 'scrim' }, dialog)
    scrim.addEventListener('click', (event) => {
      if (event.target === scrim) close(options.dismissValue)
    })
    unregister = pushBack(() => {
      scrim.remove()
      resolve(options.dismissValue)
    })
    root.append(scrim)
    options.onOpen?.(dialog, close)
  })

export const confirmDialog = (title: string, message: string, okLabel = 'OK', danger = false): Promise<boolean> =>
  showDialog({
    title,
    body: message,
    dismissValue: false,
    buttons: [
      { label: 'Cancel', value: false },
      { label: okLabel, value: true, primary: !danger, danger }
    ]
  })

export const promptDialog = async(title: string, label: string, initial = ''): Promise<string | null> => {
  const input = h('input', { type: 'text', class: 'field', value: initial, 'aria-label': label })
  let result: string | null = null
  const ok = await showDialog({
    title,
    body: h('label', { class: 'field-label' }, label, input),
    dismissValue: false,
    buttons: [
      { label: 'Cancel', value: false },
      { label: 'OK', value: true, primary: true }
    ],
    beforeClose: (value) => {
      result = input.value
      return value
    },
    onOpen: () => {
      input.focus()
      input.select()
      input.addEventListener('keydown', (event) => {
        if (event.key === 'Enter') (input.closest('.dialog')?.querySelector('.btn.primary') as HTMLButtonElement | null)?.click()
      })
    }
  })
  return ok ? result : null
}

export const choiceDialog = <T extends string>(
  title: string,
  choices: Array<{ value: T, label: string, hint?: string }>
): Promise<T | null> => {
  const list = h('div', { class: 'choice-list' })
  return showDialog<T | null>({
    title,
    body: list,
    dismissValue: null,
    buttons: [{ label: 'Cancel', value: null }],
    onOpen: (_dialog, close) => {
      for (const choice of choices) {
        list.append(h('button', { type: 'button', class: 'choice', onClick: () => close(choice.value) },
          h('span', { class: 'choice-label' }, choice.label),
          choice.hint ? h('span', { class: 'choice-hint' }, choice.hint) : null
        ))
      }
    }
  })
}
