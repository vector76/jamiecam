/**
 * Wasm-backed G-code viewer API.
 *
 * The Rust core is compiled to WebAssembly by wasm-pack (see `pnpm wasm:build`)
 * and lives in `src/wasm-pkg/`. We dynamically import it so the heavy wasm
 * module only loads when the user actually needs it.
 */

import type { AppError, GcodeViewerLoadResult } from './types'

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

function toAppError(err: unknown): AppError {
  if (err && typeof err === 'object' && 'kind' in err && 'message' in err) {
    return err as AppError
  }
  return { kind: 'Unknown', message: String(err ?? 'unknown error') }
}
