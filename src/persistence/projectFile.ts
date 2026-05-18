/**
 * `.jcam` project file format — pack and unpack.
 *
 * Layout (zip):
 *   project.json       Manifest with file name, mode, and mode-specific payload.
 *   gcode.nc           Raw G-code text (only for the `gcode-viewer` mode).
 *   imported.svg / .dxf  Original imported bytes (only for the `2d-profile` mode).
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
import type {
  CutSide,
  ParseWarning,
  Polyline,
  SetupId,
  SimulateGcodeViewerParams,
} from '../api/types'

const FORMAT_TAG = 'jamiecam-project'
const FORMAT_VERSION = 2
const SUPPORTED_READ_VERSIONS = [1, 2] as const

export type ProjectMode = 'gcode-viewer' | '2d-profile'

export interface GcodeViewerPayload {
  gcode: string
  sim: SimulateGcodeViewerParams
}

/** Imported artwork formats Mode 2 currently understands. */
export type Mode2SourceFormat = 'svg' | 'dxf'

/**
 * Form-shaped parameters for a Mode 2 profile operation. Mirrors the
 * fields edited in the Operation sidebar so the form can be hydrated
 * and saved without an extra mapping layer.
 */
export interface Mode2OperationParams {
  toolId: string | null
  cutSide: CutSide
  depthTotal: number
  depthPerPass: number
  safeZ: number
  plungeFeed: number
  cutFeed: number
  spindleRpm: number
}

/**
 * Full savable state for a Mode 2 (2-D Profile) project.
 *
 * `sourceBytes` is the unmodified file the user imported. We persist
 * them — rather than only the parsed `paths` cache — so that re-opening
 * a project lets the user re-export, re-import, or re-parse with a
 * future parser version against the exact source they started from.
 * Parsers evolve; the path cache could become stale or incompatible,
 * and without the original bytes the project would be unrecoverable.
 *
 * The bytes live in a separate zip entry (`imported.svg` /
 * `imported.dxf`) rather than base64-encoded inside `project.json`, so
 * deflate can compress them and `project.json` stays small and
 * inspectable.
 */
export interface Mode2ProfilePayload {
  sourceFormat: Mode2SourceFormat
  sourceBytes: Uint8Array
  paths: Polyline[]
  warnings: ParseWarning[]
  selectedPaths: boolean[]
  operation: Mode2OperationParams
  activeSetupId: SetupId | null
}

/**
 * The fully-restorable working state for a project. Discriminated on
 * `mode` so consumers narrow `payload` to the right shape after a single
 * mode check. This is what gets written into a `.jcam` zip and what we
 * round-trip through IndexedDB for the Recents list.
 *
 * For Mode 2 projects, `fileName` is the original imported file name
 * (e.g. `part.svg`) — the Recents list displays it directly.
 */
export type ProjectState =
  | { fileName: string; mode: 'gcode-viewer'; payload: GcodeViewerPayload }
  | { fileName: string; mode: '2d-profile'; payload: Mode2ProfilePayload }

/**
 * On-disk shape of the 2d-profile payload. Identical to
 * `Mode2ProfilePayload` minus `sourceBytes`, which is stored as a
 * separate zip entry rather than inlined.
 */
type Mode2ManifestPayload = Omit<Mode2ProfilePayload, 'sourceBytes'>

type ManifestPayload =
  | { sim: SimulateGcodeViewerParams } // gcode-viewer (gcode is in gcode.nc)
  | Mode2ManifestPayload // 2d-profile (sourceBytes is in imported.<format>)

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

function importedEntryName(format: Mode2SourceFormat): string {
  return `imported.${format}`
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
  } else {
    entries[importedEntryName(state.payload.sourceFormat)] = state.payload.sourceBytes
  }

  return zipSync(entries)
}

function manifestPayloadFor(state: ProjectState): ManifestPayload {
  switch (state.mode) {
    case 'gcode-viewer':
      return { sim: state.payload.sim }
    case '2d-profile': {
      const p = state.payload
      return {
        sourceFormat: p.sourceFormat,
        paths: p.paths,
        warnings: p.warnings,
        selectedPaths: p.selectedPaths,
        operation: p.operation,
        activeSetupId: p.activeSetupId,
      }
    }
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
      const mode2 = asMode2ManifestPayload(payload)
      const sourceBytes = entries[importedEntryName(mode2.sourceFormat)]
      if (!sourceBytes) {
        throw new JcamFormatError(
          `Missing ${importedEntryName(mode2.sourceFormat)} inside the project zip.`,
        )
      }
      return {
        fileName: manifest.fileName,
        mode: '2d-profile',
        payload: { ...mode2, sourceBytes },
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

function asMode2ManifestPayload(value: Record<string, unknown>): Mode2ManifestPayload {
  if (value.sourceFormat !== 'svg' && value.sourceFormat !== 'dxf') {
    throw new JcamFormatError(
      'Mode `2d-profile` payload has an invalid `sourceFormat`; expected "svg" or "dxf".',
    )
  }
  if (!Array.isArray(value.paths)) {
    throw new JcamFormatError('Mode `2d-profile` payload is missing `paths` array.')
  }
  if (!Array.isArray(value.warnings)) {
    throw new JcamFormatError('Mode `2d-profile` payload is missing `warnings` array.')
  }
  if (!Array.isArray(value.selectedPaths)) {
    throw new JcamFormatError(
      'Mode `2d-profile` payload is missing `selectedPaths` array.',
    )
  }
  if (!isOperationParams(value.operation)) {
    throw new JcamFormatError(
      'Mode `2d-profile` payload has an invalid `operation` block.',
    )
  }
  const activeSetupId = value.activeSetupId
  if (activeSetupId !== null && typeof activeSetupId !== 'string') {
    throw new JcamFormatError(
      'Mode `2d-profile` payload `activeSetupId` must be a string or null.',
    )
  }
  return {
    sourceFormat: value.sourceFormat,
    paths: value.paths as Polyline[],
    warnings: value.warnings as ParseWarning[],
    selectedPaths: value.selectedPaths as boolean[],
    operation: value.operation,
    activeSetupId,
  }
}

function isOperationParams(value: unknown): value is Mode2OperationParams {
  if (!value || typeof value !== 'object') return false
  const v = value as Record<string, unknown>
  const toolIdOk = v.toolId === null || typeof v.toolId === 'string'
  const cutSideOk =
    v.cutSide === 'outside' || v.cutSide === 'inside' || v.cutSide === 'onLine'
  return (
    toolIdOk &&
    cutSideOk &&
    typeof v.depthTotal === 'number' &&
    typeof v.depthPerPass === 'number' &&
    typeof v.safeZ === 'number' &&
    typeof v.plungeFeed === 'number' &&
    typeof v.cutFeed === 'number' &&
    typeof v.spindleRpm === 'number'
  )
}
