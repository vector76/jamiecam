/**
 * `.jcam` project file format — pack and unpack.
 *
 * Layout (zip):
 *   project.json    Manifest with file name, mode, and mode-specific payload.
 *   gcode.nc        The raw G-code text (only for the `gcode-viewer` mode).
 *
 * Why a zip rather than a single JSON: G-code files are often several MB
 * of mostly-repeating ASCII; deflate shrinks them ~5x. Separating the
 * manifest from the G-code also keeps the structured part inspectable
 * with `unzip -p project.jcam project.json | jq .`.
 *
 * Versioning: the manifest carries `format: "jamiecam-project"` and a
 * numeric `version`. The writer always emits the current `FORMAT_VERSION`.
 * The reader accepts every version in `SUPPORTED_READ_VERSIONS` and
 * migrates older shapes up to the current in-memory `ProjectState`.
 *
 * Mode discriminator: v2 adds a top-level `mode` so a single file format
 * can carry both Mode 1 (G-code Viewer) and Mode 2 (2-D Profile) projects.
 * V1 files predate the mode field; the reader treats them as
 * `gcode-viewer` and wraps the flat `{ sim }` manifest into the v2
 * `{ payload: { gcode, sim } }` shape.
 */

import { zipSync, unzipSync, strToU8, strFromU8 } from 'fflate'
import type { SimulateGcodeViewerParams } from '../api/types'

const FORMAT_TAG = 'jamiecam-project'
const FORMAT_VERSION = 2
const SUPPORTED_READ_VERSIONS = [1, 2] as const

export type ProjectMode = 'gcode-viewer' | '2d-profile'

export interface GcodeViewerPayload {
  gcode: string
  sim: SimulateGcodeViewerParams
}

/**
 * Placeholder for Phase 4B. The `kind` discriminator keeps the type
 * meaningfully distinct from `{}` (which TypeScript treats as "any
 * non-nullish value") and gives the reader something to validate.
 */
export interface Mode2ProfilePayload {
  kind: '2d-profile'
}

/**
 * The fully-restorable working state for a project. Discriminated on
 * `mode` so consumers narrow `payload` to the right shape after a single
 * mode check. This is what gets written into a `.jcam` zip and what we
 * round-trip through IndexedDB for the Recents list.
 */
export type ProjectState =
  | { fileName: string; mode: 'gcode-viewer'; payload: GcodeViewerPayload }
  | { fileName: string; mode: '2d-profile'; payload: Mode2ProfilePayload }

type ManifestPayload =
  | { sim: SimulateGcodeViewerParams } // gcode-viewer (gcode is in gcode.nc)
  | { kind: '2d-profile' }

/**
 * The on-disk manifest shape. `mode` is typed as `string` (not
 * `ProjectMode`) because it's parsed straight from JSON — the
 * dispatch switch in `unpackJcamProject` is what validates it
 * against the known modes.
 */
interface ProjectManifest {
  format: typeof FORMAT_TAG
  version: number
  fileName: string
  mode: string
  payload: ManifestPayload
}

export function packJcamProject(state: ProjectState): Uint8Array {
  const entries: Record<string, Uint8Array> = {}

  const manifest: ProjectManifest = {
    format: FORMAT_TAG,
    version: FORMAT_VERSION,
    fileName: state.fileName,
    mode: state.mode,
    payload: manifestPayloadFor(state),
  }
  entries['project.json'] = strToU8(JSON.stringify(manifest, null, 2))

  if (state.mode === 'gcode-viewer') {
    entries['gcode.nc'] = strToU8(state.payload.gcode)
  }

  return zipSync(entries)
}

function manifestPayloadFor(state: ProjectState): ManifestPayload {
  switch (state.mode) {
    case 'gcode-viewer':
      return { sim: state.payload.sim }
    case '2d-profile':
      return { kind: '2d-profile' }
  }
}

export class JcamFormatError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'JcamFormatError'
  }
}

export function unpackJcamProject(bytes: Uint8Array): ProjectState {
  let entries: Record<string, Uint8Array>
  try {
    entries = unzipSync(bytes)
  } catch (err) {
    throw new JcamFormatError(`Not a valid zip file: ${(err as Error).message}`)
  }

  const manifestBytes = entries['project.json']
  if (!manifestBytes) {
    throw new JcamFormatError('Missing project.json — not a JamieCam project file.')
  }

  let manifest: unknown
  try {
    manifest = JSON.parse(strFromU8(manifestBytes))
  } catch (err) {
    throw new JcamFormatError(`project.json is not valid JSON: ${(err as Error).message}`)
  }

  if (!isManifestEnvelope(manifest)) {
    throw new JcamFormatError('project.json is missing required fields or has the wrong shape.')
  }
  if (manifest.format !== FORMAT_TAG) {
    throw new JcamFormatError(`Unexpected format tag: ${manifest.format}`)
  }
  if (!isSupportedVersion(manifest.version)) {
    throw new JcamFormatError(
      `Unsupported project version ${manifest.version}; this build understands versions ${SUPPORTED_READ_VERSIONS.join(', ')}.`,
    )
  }

  if (manifest.version === 1) {
    return readV1(manifest, entries)
  }

  if (!isV2Manifest(manifest)) {
    throw new JcamFormatError('project.json is missing required fields or has the wrong shape.')
  }

  switch (manifest.mode) {
    case 'gcode-viewer': {
      const payload = manifest.payload as Record<string, unknown>
      if (!isSimParams(payload.sim)) {
        throw new JcamFormatError('Mode `gcode-viewer` payload is missing valid `sim` parameters.')
      }
      const gcodeBytes = entries['gcode.nc']
      if (!gcodeBytes) {
        throw new JcamFormatError('Missing gcode.nc inside the project zip.')
      }
      return {
        fileName: manifest.fileName,
        mode: 'gcode-viewer',
        payload: { gcode: strFromU8(gcodeBytes), sim: payload.sim },
      }
    }
    case '2d-profile': {
      const payload = manifest.payload as Record<string, unknown>
      if (payload.kind !== '2d-profile') {
        throw new JcamFormatError('Mode `2d-profile` payload has the wrong discriminator.')
      }
      return {
        fileName: manifest.fileName,
        mode: '2d-profile',
        payload: { kind: '2d-profile' },
      }
    }
    default:
      throw new JcamFormatError(`Unknown project mode: ${manifest.mode}`)
  }
}

interface ManifestEnvelope {
  format: string
  version: number
  fileName: string
}

/**
 * Minimal shape shared by every supported manifest version: enough to
 * dispatch on `format` and `version` before applying a version-specific
 * validator.
 */
function isManifestEnvelope(value: unknown): value is ManifestEnvelope {
  if (!value || typeof value !== 'object') return false
  const v = value as Record<string, unknown>
  return (
    typeof v.format === 'string' &&
    typeof v.version === 'number' &&
    typeof v.fileName === 'string'
  )
}

function isSupportedVersion(version: number): boolean {
  return (SUPPORTED_READ_VERSIONS as readonly number[]).includes(version)
}

function isV2Manifest(value: ManifestEnvelope): value is ProjectManifest {
  const v = value as unknown as Record<string, unknown>
  return typeof v.mode === 'string' && !!v.payload && typeof v.payload === 'object'
}

/**
 * V1 manifests predate the `mode` / `payload` envelope and store `sim`
 * at the top level alongside `fileName`. There was only ever one mode
 * (the G-code Viewer), so migration is mechanical: read `sim` from the
 * manifest, read `gcode.nc` from the zip, and return the v2-shaped
 * `gcode-viewer` state.
 */
function readV1(
  manifest: ManifestEnvelope,
  entries: Record<string, Uint8Array>,
): ProjectState {
  const raw = manifest as unknown as Record<string, unknown>
  if (!isSimParams(raw.sim)) {
    throw new JcamFormatError('Legacy v1 project.json is missing valid `sim` parameters.')
  }
  const gcodeBytes = entries['gcode.nc']
  if (!gcodeBytes) {
    throw new JcamFormatError('Missing gcode.nc inside the project zip.')
  }
  return {
    fileName: manifest.fileName,
    mode: 'gcode-viewer',
    payload: { gcode: strFromU8(gcodeBytes), sim: raw.sim },
  }
}

function isSimParams(value: unknown): value is SimulateGcodeViewerParams {
  if (!value || typeof value !== 'object') return false
  const v = value as Record<string, unknown>
  const stock = v.stock as Record<string, unknown> | undefined
  if (!stock || typeof stock !== 'object') return false
  const origin = stock.origin as Record<string, unknown> | undefined
  return (
    typeof v.toolDiameter === 'number' &&
    typeof v.resolution === 'number' &&
    typeof stock.width === 'number' &&
    typeof stock.depth === 'number' &&
    typeof stock.height === 'number' &&
    !!origin &&
    typeof origin === 'object' &&
    typeof origin.x === 'number' &&
    typeof origin.y === 'number' &&
    typeof origin.z === 'number'
  )
}
