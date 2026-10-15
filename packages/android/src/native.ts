// Promise wrapper around the Java bridge (Bridge.java). Each call is a JSON
// message `{ id, method, args }`; Java answers with `{ id, ok, result | error }`
// and pushes unsolicited events (pause, back, open) through
// `window.__markdNative.event(name, payload)`.

export interface DocumentInfo {
  uri: string
  name: string
  text: string
  /** True when the folder holding the document is writable (tree grant). */
  hasFolder: boolean
}

export interface TreeEntry {
  name: string
  uri: string
  docId: string
  isDir: boolean
  modified: number
}

export interface HostInfo {
  sdkInt: number
  release: string
  model: string
  version: string
  handwritingSupported: boolean
}

// Injected by androidx.webkit `addWebMessageListener`, only into the app's own
// origin and only answered for the main frame, so documents (iframes, HTML
// blocks) cannot reach the file APIs.
interface NativeHost {
  postMessage: (message: string) => void
  onmessage: ((event: MessageEvent<string>) => void) | null
}

interface NativeReply {
  id: string
  ok: boolean
  result?: unknown
  error?: string
}

type EventHandler = (payload: unknown) => unknown

/** Entry points MainActivity reaches with `evaluateJavascript`. */
interface NativeReceiver {
  event: (name: string, payload: unknown) => unknown
}

declare global {
  interface Window {
    MarkdHost?: NativeHost
    __markdNative: NativeReceiver
  }
}

const pending = new Map<string, { resolve: (value: unknown) => void, reject: (err: Error) => void }>()
const handlers = new Map<string, EventHandler>()
let nextId = 1

window.__markdNative = {
  event(name, payload) {
    const handler = handlers.get(name)
    return handler ? handler(payload) : undefined
  }
}

if (window.MarkdHost) {
  window.MarkdHost.onmessage = (event) => {
    const reply = JSON.parse(event.data) as NativeReply
    const entry = pending.get(reply.id)
    if (!entry) return
    pending.delete(reply.id)
    if (reply.ok) entry.resolve(reply.result)
    else entry.reject(new NativeError(reply.error ?? 'ERROR: unknown'))
  }
}

export class NativeError extends Error {
  /** `CANCELLED` when the user dismissed a system picker. */
  get code(): string {
    return this.message.split(':')[0] ?? ''
  }
}

export const isNative = (): boolean => !!window.MarkdHost

export const callNative = <T = unknown>(method: string, args: Record<string, unknown> = {}): Promise<T> => {
  const host = window.MarkdHost
  if (!host) return Promise.reject(new NativeError('UNAVAILABLE: not running inside the Android app'))
  const id = String(nextId++)
  return new Promise<T>((resolve, reject) => {
    pending.set(id, { resolve: resolve as (value: unknown) => void, reject })
    host.postMessage(JSON.stringify({ id, method, args }))
  })
}

export const onNativeEvent = (name: string, handler: EventHandler): void => {
  handlers.set(name, handler)
}

export const isCancelled = (err: unknown): boolean => err instanceof NativeError && err.code === 'CANCELLED'

export const blobToBase64 = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result).replace(/^data:[^,]*,/, ''))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })

export const blobToDataUrl = (blob: Blob): Promise<string> =>
  new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(blob)
  })
