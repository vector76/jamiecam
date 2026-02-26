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
