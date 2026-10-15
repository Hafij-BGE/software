// Craft Suite's side of the Markd page (see src/markd/mod.rs), run before the page's own scripts.
// It provides the `MarkdHost` the page's native.ts looks for (the Android app's is injected by
// androidx.webkit), reports the document's name and unsaved state, and hands the suite's tab
// shortcuts to the window.
//
// Every message carries this tab's secret (`token`, filled in by the suite), which only this
// script knows: the suite drops messages without it, such as ones from frames inside a document.
(() => {
  if (window.top !== window || window.MarkdHost) return
  const ipc = window.ipc
  const token = '@TOKEN@'
  const send = (message) => ipc.postMessage(JSON.stringify({ t: token, m: message }))
  const post = (message) => send(JSON.stringify(message))

  window.MarkdHost = { postMessage: (message) => send(String(message)), onmessage: null }
  // Answers to the page's calls.
  window.__craftSuiteReply = (data) => {
    const handler = window.MarkdHost.onmessage
    if (handler) handler({ data })
  }
  // Events from the suite; their (possibly asynchronous) answers go back as `eventResult`.
  window.__craftSuiteEvent = (name, payload) => {
    const native = window.__markdNative
    Promise.resolve(native ? native.event(name, payload) : undefined).then(
      (result) => post({ suite: 'eventResult', name, result: result === undefined ? null : result }),
      () => post({ suite: 'eventResult', name, result: null })
    )
  }

  // Each tab is one document: a new tab starts blank instead of reopening the last file or the
  // unsaved draft another tab left (the recent files stay).
  try {
    localStorage.removeItem('markd.last')
    localStorage.removeItem('markd.draft')
  } catch (_) {}

  const report = () => {
    const name = document.getElementById('doc-name')
    const dot = document.getElementById('dirty-dot')
    if (name) post({ suite: 'state', name: name.textContent || 'Untitled.md', dirty: !!dot && !dot.hidden })
  }
  document.addEventListener('DOMContentLoaded', () => {
    const observer = new MutationObserver(report)
    for (const id of ['doc-name', 'dirty-dot']) {
      const el = document.getElementById(id)
      if (el) observer.observe(el, { attributes: true, childList: true, characterData: true, subtree: true })
    }
    report()
  })

  // The suite's shortcuts (Settings › Behaviour › Keyboard), which the window can't see while
  // the page has the keyboard.
  document.addEventListener('keydown', (e) => {
    let action = null
    if (e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey && (e.key === 'n' || e.key === 'N')) action = 'new'
    else if (e.ctrlKey && e.altKey) {
      if (e.key === 'PageDown') action = e.shiftKey ? 'moveNext' : 'next'
      else if (e.key === 'PageUp') action = e.shiftKey ? 'movePrev' : 'prev'
      else if (e.key === 'Insert') action = 'newDefault'
      else if (e.key === 'F4') action = 'close'
      else if (e.key === 'Home') action = 'home'
    }
    if (!action) return
    e.preventDefault()
    e.stopPropagation()
    post({ suite: 'key', action })
  }, true)

  document.addEventListener('mousedown', () => post({ suite: 'focus' }), true)
})()
