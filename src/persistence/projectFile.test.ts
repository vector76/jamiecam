import {
  packJcamProject,
  unpackJcamProject,
  JcamFormatError,
  type ProjectState,
} from './projectFile'
import { strToU8, strFromU8, unzipSync, zipSync } from 'fflate'

const SAMPLE: ProjectState = {
  fileName: 'demo.nc',
  mode: 'gcode-viewer',
  payload: {
    gcode: '; @STOCK type=box width=100 depth=50 height=10\nG0 X0 Y0\nG1 Z-1 F200\n',
    sim: {
      stock: {
        origin: { x: 0, y: 0, z: 0 },
        width: 100,
        depth: 50,
        height: 10,
      },
      toolDiameter: 6,
      resolution: 0.5,
    },
  },
}

describe('packJcamProject / unpackJcamProject', () => {
  it('round-trips a gcode-viewer project state', () => {
    const packed = packJcamProject(SAMPLE)
    const unpacked = unpackJcamProject(packed)
    expect(unpacked).toEqual(SAMPLE)
  })

  it('writes the mode discriminator into the manifest', () => {
    const packed = packJcamProject(SAMPLE)
    const entries = unzipSync(packed)
    const manifest = JSON.parse(strFromU8(entries['project.json']))
    expect(manifest.mode).toBe('gcode-viewer')
    expect(manifest.version).toBe(2)
  })

  it('round-trips a 2d-profile project state (placeholder payload)', () => {
    const state: ProjectState = {
      fileName: 'part.svg',
      mode: '2d-profile',
      payload: { kind: '2d-profile' },
    }
    const packed = packJcamProject(state)
    const unpacked = unpackJcamProject(packed)
    expect(unpacked).toEqual(state)
  })

  it('does not write gcode.nc for a 2d-profile project', () => {
    const state: ProjectState = {
      fileName: 'part.svg',
      mode: '2d-profile',
      payload: { kind: '2d-profile' },
    }
    const entries = unzipSync(packJcamProject(state))
    expect(entries['gcode.nc']).toBeUndefined()
  })

  it('preserves unicode in the G-code (the file is UTF-8)', () => {
    const project: ProjectState = {
      ...SAMPLE,
      payload: { ...SAMPLE.payload, gcode: '; café — Schöne Grüße\nG0 X0\n' },
    }
    const unpacked = unpackJcamProject(packJcamProject(project))
    if (unpacked.mode !== 'gcode-viewer') throw new Error('expected gcode-viewer mode')
    expect(unpacked.payload.gcode).toBe('; café — Schöne Grüße\nG0 X0\n')
  })

  it('throws JcamFormatError on a non-zip blob', () => {
    expect(() => unpackJcamProject(strToU8('hello world'))).toThrow(JcamFormatError)
  })

  it('throws when project.json is missing from the zip', () => {
    const bad = zipSync({ 'gcode.nc': strToU8('G0 X0') })
    expect(() => unpackJcamProject(bad)).toThrow(/Missing project\.json/)
  })

  it('throws when gcode.nc is missing for a gcode-viewer project', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 2,
          fileName: 'x.nc',
          mode: 'gcode-viewer',
          payload: { sim: SAMPLE.payload.sim },
        }),
      ),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/Missing gcode\.nc/)
  })

  it('throws when project.json is invalid JSON', () => {
    const bad = zipSync({
      'project.json': strToU8('not json'),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/not valid JSON/)
  })

  it('throws when the manifest is missing required fields', () => {
    const bad = zipSync({
      'project.json': strToU8(JSON.stringify({ format: 'jamiecam-project', version: 2 })),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/missing required fields/)
  })

  it('throws on an unsupported manifest version (including legacy v1)', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 1,
          fileName: 'x.nc',
          mode: 'gcode-viewer',
          payload: { sim: SAMPLE.payload.sim },
        }),
      ),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/Unsupported project version/)
  })

  it('throws on a foreign format tag', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'something-else',
          version: 2,
          fileName: 'x.nc',
          mode: 'gcode-viewer',
          payload: { sim: SAMPLE.payload.sim },
        }),
      ),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/Unexpected format tag/)
  })

  it('throws on an unknown mode', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 2,
          fileName: 'x.nc',
          mode: 'flux-capacitor',
          payload: {},
        }),
      ),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/Unknown project mode/)
  })

  it('throws when the gcode-viewer payload is missing sim', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 2,
          fileName: 'x.nc',
          mode: 'gcode-viewer',
          payload: {},
        }),
      ),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/sim/)
  })
})
