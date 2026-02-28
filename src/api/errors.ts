import { invoke } from '@tauri-apps/api/core'
import type { AppError } from './types'

/**
 * Narrow an unknown rejection value to a typed AppError.
 *
 * Tauri serialises Rust errors as `{ kind, message }` objects. Any other
 * shape (e.g. a network error during development) is wrapped in an
 * AppError with kind "Unknown".
 */
export function toAppError(e: unknown): AppError {
  if (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    typeof (e as Record<string, unknown>).kind === 'string'
  ) {
    return e as AppError
  }
  return { kind: 'Unknown', message: String(e) }
}

/**
 * Invoke a Tauri IPC command and convert any rejection to a typed AppError.
 *
 * This is a thin wrapper around `invoke` that eliminates the repetitive
 * try/catch/toAppError boilerplate in every API module.
 */
export async function typedInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw toAppError(e)
  }
}
