import {
  packJcamProject,
  unpackJcamProject,
  JcamFormatError,
  type ProjectState,
} from './projectFile'
import { strToU8, zipSync } from 'fflate'

const SAMPLE: ProjectState = {
  fileName: 'demo.nc',
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
}

describe('packJcamProject / unpackJcamProject', () => {
  it('round-trips a project state', () => {
    const packed = packJcamProject(SAMPLE)
    const unpacked = unpackJcamProject(packed)
    expect(unpacked).toEqual(SAMPLE)
  })

  it('preserves unicode in the G-code (the file is UTF-8)', () => {
    const project: ProjectState = {
      ...SAMPLE,
      gcode: '; café — Schöne Grüße\nG0 X0\n',
    }
    const unpacked = unpackJcamProject(packJcamProject(project))
    expect(unpacked.gcode).toBe(project.gcode)
  })

  it('throws JcamFormatError on a non-zip blob', () => {
    expect(() => unpackJcamProject(strToU8('hello world'))).toThrow(JcamFormatError)
  })

  it('throws when project.json is missing from the zip', () => {
    const bad = zipSync({ 'gcode.nc': strToU8('G0 X0') })
    expect(() => unpackJcamProject(bad)).toThrow(/Missing project\.json/)
  })

  it('throws when gcode.nc is missing from the zip', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 1,
          fileName: 'x.nc',
          sim: SAMPLE.sim,
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
      'project.json': strToU8(JSON.stringify({ format: 'jamiecam-project', version: 1 })),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/missing required fields/)
  })

  it('throws on an unsupported manifest version', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 999,
          fileName: 'x.nc',
          sim: SAMPLE.sim,
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
          version: 1,
          fileName: 'x.nc',
          sim: SAMPLE.sim,
        }),
      ),
      'gcode.nc': strToU8('G0 X0'),
    })
    expect(() => unpackJcamProject(bad)).toThrow(/Unexpected format tag/)
  })
})
