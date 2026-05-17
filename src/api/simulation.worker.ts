/**
 * Dedicated Web Worker that runs the dexel material-removal simulation
 * off the main thread.
 *
 * The worker owns its own WebAssembly.Instance — the wasm Module bytes
 * are cached by the browser, but each thread needs its own instance.
 * Initialization (`init()`) is performed lazily on the first message
 * so the cost is paid only when the user actually triggers a sim.
 *
 * Wire format: see `SimWorkerRequest` / `SimWorkerResponse` below.
 *
 * We deliberately do NOT pull in TypeScript's `WebWorker` lib — adding it
 * (via tsconfig or a triple-slash reference) globally shadows the DOM
 * event maps for the rest of the project. Instead we describe the bits
 * of the worker scope we use with a local interface and cast `self` to it.
 */

import init, { simulateGcodeViewer } from '../wasm-pkg/jamiecam'
import type { AppError, MeshData, SimulateGcodeViewerParams } from './types'

export interface SimWorkerRequest {
  id: number
  content: string
  params: SimulateGcodeViewerParams
}

export type SimWorkerResponse =
  | { id: number; ok: true; mesh: MeshData }
  | { id: number; ok: false; error: AppError }

interface WorkerScope {
  postMessage(message: SimWorkerResponse): void
  addEventListener(
    type: 'message',
    listener: (event: MessageEvent<SimWorkerRequest>) => void,
  ): void
}
const scope = self as unknown as WorkerScope

let ready: Promise<void> | null = null
function ensureReady(): Promise<void> {
  if (!ready) {
    ready = init().then(() => undefined)
  }
  return ready
}

function toAppError(err: unknown): AppError {
  if (err && typeof err === 'object' && 'kind' in err && 'message' in err) {
    return err as AppError
  }
  return { kind: 'Unknown', message: String(err ?? 'unknown error') }
}

scope.addEventListener('message', async (event) => {
  const { id, content, params } = event.data
  try {
    await ensureReady()
    const mesh = simulateGcodeViewer(content, params) as MeshData
    scope.postMessage({ id, ok: true, mesh })
  } catch (err) {
    scope.postMessage({ id, ok: false, error: toAppError(err) })
  }
})
