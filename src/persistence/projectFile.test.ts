import {
  packJcamProject,
  unpackJcamProject,
  JcamFormatError,
  type Mode2ProfilePayload,
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

  describe('2d-profile round trip', () => {
    const SVG_BYTES = new Uint8Array([0x3c, 0x73, 0x76, 0x67, 0x2f, 0x3e]) // "<svg/>"
    const DXF_BYTES = new Uint8Array([0x30, 0x0a, 0x45, 0x4f, 0x46, 0x0a]) // "0\nEOF\n"

    const FULL_PAYLOAD: Mode2ProfilePayload = {
      sourceFormat: 'svg',
      sourceBytes: SVG_BYTES,
      paths: [
        {
          closed: true,
          points: [
            { x: 0, y: 0 },
            { x: 10, y: 0 },
            { x: 10, y: 10 },
            { x: 0, y: 10 },
          ],
        },
        {
          closed: false,
          points: [
            { x: 1, y: 1 },
            { x: 5, y: 5 },
          ],
        },
      ],
      warnings: [
        { line: 7, message: 'skipped <text>' },
        { line: null, message: 'unsupported transform' },
      ],
      selectedPaths: [true, false],
      operation: {
        toolId: 'tool-1',
        cutSide: 'inside',
        depthTotal: 7.5,
        depthPerPass: 1.5,
        safeZ: 6,
        plungeFeed: 250,
        cutFeed: 900,
        spindleRpm: 20000,
      },
      activeSetupId: 'setup-1',
    }

    it('round-trips a Mode 2 project end-to-end', () => {
      const state: ProjectState = {
        fileName: 'shape.svg',
        mode: '2d-profile',
        payload: FULL_PAYLOAD,
      }
      const unpacked = unpackJcamProject(packJcamProject(state))
      expect(unpacked).toEqual(state)
    })

    it('stores original SVG bytes in imported.svg and writes no gcode.nc', () => {
      const state: ProjectState = {
        fileName: 'shape.svg',
        mode: '2d-profile',
        payload: FULL_PAYLOAD,
      }
      const entries = unzipSync(packJcamProject(state))
      expect(entries['imported.svg']).toEqual(SVG_BYTES)
      expect(entries['imported.dxf']).toBeUndefined()
      expect(entries['gcode.nc']).toBeUndefined()
    })

    it('uses imported.dxf when the source format is DXF', () => {
      const state: ProjectState = {
        fileName: 'part.dxf',
        mode: '2d-profile',
        payload: { ...FULL_PAYLOAD, sourceFormat: 'dxf', sourceBytes: DXF_BYTES },
      }
      const entries = unzipSync(packJcamProject(state))
      expect(entries['imported.dxf']).toEqual(DXF_BYTES)
      expect(entries['imported.svg']).toBeUndefined()
    })

    it('keeps sourceBytes out of project.json', () => {
      const state: ProjectState = {
        fileName: 'shape.svg',
        mode: '2d-profile',
        payload: FULL_PAYLOAD,
      }
      const entries = unzipSync(packJcamProject(state))
      const manifest = JSON.parse(strFromU8(entries['project.json']))
      expect(manifest.mode).toBe('2d-profile')
      expect(manifest.payload.sourceFormat).toBe('svg')
      expect(manifest.payload).not.toHaveProperty('sourceBytes')
    })

    it('throws when the imported.<format> entry is missing', () => {
      const bad = zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 2,
            fileName: 'shape.svg',
            mode: '2d-profile',
            payload: {
              sourceFormat: 'svg',
              paths: FULL_PAYLOAD.paths,
              warnings: FULL_PAYLOAD.warnings,
              selectedPaths: FULL_PAYLOAD.selectedPaths,
              operation: FULL_PAYLOAD.operation,
              activeSetupId: FULL_PAYLOAD.activeSetupId,
            },
          }),
        ),
      })
      expect(() => unpackJcamProject(bad)).toThrow(/imported\.svg/)
    })

    it('throws when sourceFormat is missing or unrecognised', () => {
      const bad = zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 2,
            fileName: 'shape.svg',
            mode: '2d-profile',
            payload: {
              sourceFormat: 'pdf',
              paths: [],
              warnings: [],
              selectedPaths: [],
              operation: FULL_PAYLOAD.operation,
              activeSetupId: null,
            },
          }),
        ),
        'imported.pdf': strToU8('%PDF-1.7'),
      })
      expect(() => unpackJcamProject(bad)).toThrow(/sourceFormat/)
    })

    it('throws when operation params are malformed', () => {
      const bad = zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 2,
            fileName: 'shape.svg',
            mode: '2d-profile',
            payload: {
              sourceFormat: 'svg',
              paths: [],
              warnings: [],
              selectedPaths: [],
              operation: { toolId: 'x', cutSide: 'spiral' },
              activeSetupId: null,
            },
          }),
        ),
        'imported.svg': strToU8('<svg/>'),
      })
      expect(() => unpackJcamProject(bad)).toThrow(/operation/)
    })
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

  it('throws on a future manifest version this build does not know', () => {
    const bad = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 99,
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

  it('attaches a typed UnknownProjectMode AppError on the thrown JcamFormatError', () => {
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
    try {
      unpackJcamProject(bad)
      throw new Error('expected unpackJcamProject to throw')
    } catch (err) {
      expect(err).toBeInstanceOf(JcamFormatError)
      const jcamErr = err as JcamFormatError
      expect(jcamErr.appError).toEqual({
        kind: 'UnknownProjectMode',
        message: { mode: 'flux-capacitor' },
      })
    }
  })

  describe('legacy v1 reader', () => {
    const V1_SIM = SAMPLE.payload.sim
    const V1_GCODE = '; legacy v1 file\nG0 X0 Y0\nG1 Z-1 F200\n'

    function v1Zip(extra: Record<string, unknown> = {}): Uint8Array {
      return zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 1,
            fileName: 'legacy.nc',
            sim: V1_SIM,
            ...extra,
          }),
        ),
        'gcode.nc': strToU8(V1_GCODE),
      })
    }

    it('migrates a v1 manifest to a gcode-viewer ProjectState', () => {
      const unpacked = unpackJcamProject(v1Zip())
      expect(unpacked).toEqual({
        fileName: 'legacy.nc',
        mode: 'gcode-viewer',
        payload: { gcode: V1_GCODE, sim: V1_SIM },
      })
    })

    it('defaults mode to gcode-viewer even if a stray mode field is present', () => {
      // V1 predates `mode`; readers must ignore anything that sneaks in
      // rather than trusting it (the v1 file format only ever meant
      // Mode 1 / G-code Viewer).
      const unpacked = unpackJcamProject(v1Zip({ mode: 'flux-capacitor' }))
      expect(unpacked.mode).toBe('gcode-viewer')
    })

    it('throws when a v1 manifest is missing sim', () => {
      const bad = zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 1,
            fileName: 'legacy.nc',
          }),
        ),
        'gcode.nc': strToU8(V1_GCODE),
      })
      expect(() => unpackJcamProject(bad)).toThrow(/sim/)
    })

    it('throws when a v1 zip is missing gcode.nc', () => {
      const bad = zipSync({
        'project.json': strToU8(
          JSON.stringify({
            format: 'jamiecam-project',
            version: 1,
            fileName: 'legacy.nc',
            sim: V1_SIM,
          }),
        ),
      })
      expect(() => unpackJcamProject(bad)).toThrow(/Missing gcode\.nc/)
    })
  })

  it('reads a hand-built v2 manifest (the same shape bead-1\'s writer emits)', () => {
    const bytes = zipSync({
      'project.json': strToU8(
        JSON.stringify({
          format: 'jamiecam-project',
          version: 2,
          fileName: 'demo.nc',
          mode: 'gcode-viewer',
          payload: { sim: SAMPLE.payload.sim },
        }),
      ),
      'gcode.nc': strToU8(SAMPLE.payload.gcode),
    })
    expect(unpackJcamProject(bytes)).toEqual(SAMPLE)
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
