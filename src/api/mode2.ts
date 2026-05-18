/**
 * Wasm-backed Mode 2 (2D Profile Cuts) API.
 *
 * Thin TypeScript wrappers around the four wasm exports added for Mode 2:
 *   - `parseSvg` / `parseDxf`  — import 2D paths from vector sources
 *   - `generateProfileToolpath` — run the profile planner
 *   - `emitGrblGcode`          — render planner output as GRBL G-code
 *
 * Mirrors the structure of `./gcodeViewer.ts`: the wasm module is
 * dynamically imported on first use; failures surface as `AppError`-shaped
 * rejections so the UI can pattern-match on the `kind` discriminant.
 */

import type {
  AppError,
  BoxDimensions,
  ParseDxfResult,
  ParseSvgResult,
  ProfileOperationInput,
  Tool,
  ToolpathOutput,
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
        wasmPromise = null
        throw err
      }
    })()
  }
  return wasmPromise
}

/** Parse an SVG document into 2D polylines (mm). */
export async function parseSvg(bytes: Uint8Array): Promise<ParseSvgResult> {
  const wasm = await getWasm()
  try {
    return wasm.parseSvg(bytes) as ParseSvgResult
  } catch (err) {
    throw toAppError(err)
  }
}

/** Parse a DXF document into 2D polylines (mm). */
export async function parseDxf(bytes: Uint8Array): Promise<ParseDxfResult> {
  const wasm = await getWasm()
  try {
    return wasm.parseDxf(bytes) as ParseDxfResult
  } catch (err) {
    throw toAppError(err)
  }
}

/** Run the profile planner, returning an ordered toolpath. */
export async function generateProfileToolpath(
  input: ProfileOperationInput,
): Promise<ToolpathOutput> {
  const wasm = await getWasm()
  try {
    return wasm.generateProfileToolpath(input) as ToolpathOutput
  } catch (err) {
    throw toAppError(err)
  }
}

/** Render a planner-generated toolpath as a GRBL G-code program. */
export async function emitGrblGcode(
  toolpath: ToolpathOutput,
  tool: Tool,
  stock: BoxDimensions,
): Promise<string> {
  const wasm = await getWasm()
  try {
    return wasm.emitGrblGcode(toolpath, tool, stock) as string
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
