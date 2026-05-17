/**
 * `.jcam` project file format — pack and unpack.
 *
 * Layout (zip):
 *   project.json    Manifest with file name and simulation parameters.
 *   gcode.nc        The raw G-code text exactly as the user loaded it.
 *
 * Why a zip rather than a single JSON: G-code files are often several MB
 * of mostly-repeating ASCII; deflate shrinks them ~5x. Separating the
 * manifest from the G-code also keeps the structured part inspectable
 * with `unzip -p project.jcam project.json | jq .`.
 *
 * Versioning: the manifest carries `format: "jamiecam-project"` and a
 * numeric `version`. Bump `version` when the manifest shape changes
 * incompatibly; readers should refuse anything they don't recognise.
 */

import { zipSync, unzipSync, strToU8, strFromU8 } from 'fflate'
import type { SimulateGcodeViewerParams } from '../api/types'

const FORMAT_TAG = 'jamiecam-project'
const FORMAT_VERSION = 1

/**
 * The fully-restorable working state for the G-code Viewer mode. This is
 * what gets written into a `.jcam` zip and what we round-trip through
 * IndexedDB for the Recents list.
 */
export interface ProjectState {
  fileName: string
  gcode: string
  sim: SimulateGcodeViewerParams
}

interface ProjectManifest {
  format: typeof FORMAT_TAG
  version: number
  fileName: string
  sim: SimulateGcodeViewerParams
}

export function packJcamProject(state: ProjectState): Uint8Array {
  const manifest: ProjectManifest = {
    format: FORMAT_TAG,
    version: FORMAT_VERSION,
    fileName: state.fileName,
    sim: state.sim,
  }
  return zipSync({
    'project.json': strToU8(JSON.stringify(manifest, null, 2)),
    'gcode.nc': strToU8(state.gcode),
  })
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

  if (!isManifest(manifest)) {
    throw new JcamFormatError('project.json is missing required fields or has the wrong shape.')
  }
  if (manifest.format !== FORMAT_TAG) {
    throw new JcamFormatError(`Unexpected format tag: ${manifest.format}`)
  }
  if (manifest.version !== FORMAT_VERSION) {
    throw new JcamFormatError(
      `Unsupported project version ${manifest.version}; this build understands version ${FORMAT_VERSION}.`,
    )
  }

  const gcodeBytes = entries['gcode.nc']
  if (!gcodeBytes) {
    throw new JcamFormatError('Missing gcode.nc inside the project zip.')
  }

  return {
    fileName: manifest.fileName,
    gcode: strFromU8(gcodeBytes),
    sim: manifest.sim,
  }
}

function isManifest(value: unknown): value is ProjectManifest {
  if (!value || typeof value !== 'object') return false
  const v = value as Record<string, unknown>
  return (
    typeof v.format === 'string' &&
    typeof v.version === 'number' &&
    typeof v.fileName === 'string' &&
    isSimParams(v.sim)
  )
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
