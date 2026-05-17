/**
 * Wasm-backed G-code viewer API.
 *
 * Two execution paths:
 *  - `loadGcodeForViewer` runs on the main thread because the parsed line
 *    geometry is needed synchronously to feed the viewport.
 *  - `simulateGcodeViewer` is offloaded to a dedicated Web Worker so the
 *    UI stays responsive during multi-second dexel simulations.
 *
 * The wasm module is dynamically imported on first use; the worker owns
 * its own independent instance (the compiled module bytes are cached by
 * the browser, so this only costs one extra instantiation).
 */

import SimulationWorker from './simulation.worker?worker'
import { createSimulationClient, type SimulationClient } from './simulationWorkerClient'
import type {
  AppError,
  GcodeViewerLoadResult,
  MeshData,
  SimulateGcodeViewerParams,
} from './types'

type WasmModule = typeof import('../wasm-pkg/jamiecam')

let wasmPromise: Promise<WasmModule> | null = null

async function getWasm(): Promise<WasmModule> {
  if (!wasmPromise) {
    wasmPromise = (async () => {
      try {
        const mod = await import('../wasm-pkg/jamiecam')
        await mod.default()
        return mod
      } catch (err) {
        // Clear the cache so a subsequent call can retry rather than reusing
        // the rejected promise forever.
        wasmPromise = null
        throw err
      }
    })()
  }
  return wasmPromise
}

let simClient: SimulationClient | null = null

function getSimClient(): SimulationClient {
  if (!simClient) {
    const worker = new SimulationWorker()
    // If the worker crashes, the cached client is pointing at a dead
    // worker — any subsequent simulate() would post to it and the
    // promise would hang forever. Clear the cache so the next call
    // spawns a fresh worker. The client's own error listener already
    // rejects the in-flight requests with a WorkerError.
    worker.addEventListener('error', () => {
      simClient = null
    })
    simClient = createSimulationClient(worker)
  }
  return simClient
}

/**
 * Eagerly fetch + instantiate the wasm module without performing any work.
 *
 * Lets the UI display an "initializing engine" indicator at startup so the
 * wasm download isn't hidden behind a button click. Idempotent — subsequent
 * callers reuse the same promise.
 */
export async function prewarmWasm(): Promise<void> {
  await getWasm()
}

/**
 * Parse G-code text, returning header metadata, viewport line geometry,
 * and any non-fatal warnings. Throws an `AppError`-shaped object on failure.
 */
export async function loadGcodeForViewer(content: string): Promise<GcodeViewerLoadResult> {
  const wasm = await getWasm()
  try {
    return wasm.loadGcodeForViewer(content) as GcodeViewerLoadResult
  } catch (err) {
    throw toAppError(err)
  }
}

/**
 * Run a dexel material-removal simulation on the supplied G-code, returning
 * the resulting workpiece mesh. Runs off the main thread in a Web Worker.
 * Throws an `AppError`-shaped object on failure.
 */
export async function simulateGcodeViewer(
  content: string,
  params: SimulateGcodeViewerParams,
): Promise<MeshData> {
  return getSimClient().simulate(content, params)
}

function toAppError(err: unknown): AppError {
  if (err && typeof err === 'object' && 'kind' in err && 'message' in err) {
    return err as AppError
  }
  return { kind: 'Unknown', message: String(err ?? 'unknown error') }
}
