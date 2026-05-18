/**
 * Tests for the Mode 2 wasm bridge in `./mode2.ts`.
 *
 * The wasm module is mocked at the module-loader level so the tests run
 * synchronously in jsdom without instantiating the real wasm binary —
 * same approach used for other wasm-adjacent units in this directory.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest'

import type {
  BoxDimensions,
  ParseDxfResult,
  ParseSvgResult,
  Polyline,
  ProfileOperationInput,
  Tool,
  ToolpathOutput,
} from './types'

const mockParseSvg = vi.fn()
const mockParseDxf = vi.fn()
const mockGenerateProfileToolpath = vi.fn()
const mockEmitGrblGcode = vi.fn()
const mockInit = vi.fn(async () => undefined)

vi.mock('../wasm-pkg/jamiecam', () => ({
  default: mockInit,
  parseSvg: (...args: unknown[]) => mockParseSvg(...args),
  parseDxf: (...args: unknown[]) => mockParseDxf(...args),
  generateProfileToolpath: (...args: unknown[]) => mockGenerateProfileToolpath(...args),
  emitGrblGcode: (...args: unknown[]) => mockEmitGrblGcode(...args),
}))

// Import after `vi.mock` so the bridge picks up the mocked wasm module.
import { emitGrblGcode, generateProfileToolpath, parseDxf, parseSvg } from './mode2'

const SAMPLE_TOOL: Tool = {
  id: 't1',
  name: '1/8" flat',
  diameter: 3.175,
  fluteCount: 2,
  length: 38,
  material: 'carbide',
  recommended: { spindleRpm: 18000, feedRate: 800, plungeRate: 200 },
}

const SAMPLE_STOCK: BoxDimensions = {
  origin: { x: 0, y: 0, z: 0 },
  width: 100,
  depth: 50,
  height: 10,
}

const SQUARE: Polyline = {
  points: [
    { x: 0, y: 0 },
    { x: 10, y: 0 },
    { x: 10, y: 10 },
    { x: 0, y: 10 },
  ],
  closed: true,
}

const SAMPLE_PROFILE_INPUT: ProfileOperationInput = {
  boundaries: [SQUARE],
  tool: SAMPLE_TOOL,
  cutSide: 'outside',
  depthTotal: 3,
  depthPerPass: 1.5,
  safeZ: 5,
  plungeFeed: 200,
  cutFeed: 800,
  spindleRpm: 18000,
}

beforeEach(() => {
  mockParseSvg.mockReset()
  mockParseDxf.mockReset()
  mockGenerateProfileToolpath.mockReset()
  mockEmitGrblGcode.mockReset()
  mockInit.mockClear()
})

describe('parseSvg', () => {
  it('forwards the byte payload to wasm and returns the parsed result', async () => {
    const expected: ParseSvgResult = { paths: [SQUARE], warnings: [] }
    mockParseSvg.mockReturnValue(expected)

    const bytes = new Uint8Array([1, 2, 3])
    const result = await parseSvg(bytes)

    expect(mockParseSvg).toHaveBeenCalledTimes(1)
    expect(mockParseSvg).toHaveBeenCalledWith(bytes)
    expect(result).toBe(expected)
  })

  it('rethrows wasm AppError payloads unchanged', async () => {
    mockParseSvg.mockImplementation(() => {
      throw {
        kind: 'ParseFailure',
        message: { source: 'svg', message: 'bad path', line: null },
      }
    })

    await expect(parseSvg(new Uint8Array())).rejects.toEqual({
      kind: 'ParseFailure',
      message: { source: 'svg', message: 'bad path', line: null },
    })
  })

  it('wraps non-AppError throws in an Unknown AppError', async () => {
    mockParseSvg.mockImplementation(() => {
      throw new Error('boom')
    })

    await expect(parseSvg(new Uint8Array())).rejects.toMatchObject({
      kind: 'Unknown',
    })
  })
})

describe('parseDxf', () => {
  it('forwards bytes and returns the parsed paths + warnings', async () => {
    const expected: ParseDxfResult = {
      paths: [SQUARE],
      warnings: [{ line: 4, message: 'unsupported entity SPLINE' }],
    }
    mockParseDxf.mockReturnValue(expected)

    const bytes = new Uint8Array([0])
    const result = await parseDxf(bytes)

    expect(mockParseDxf).toHaveBeenCalledWith(bytes)
    expect(result).toBe(expected)
  })
})

describe('generateProfileToolpath', () => {
  it('forwards the input verbatim and returns the toolpath', async () => {
    const expected: ToolpathOutput = [
      { kind: 'rapid', to: [0, 0, 5] },
      { kind: 'linear', to: [10, 0, -1], feed: 800 },
    ]
    mockGenerateProfileToolpath.mockReturnValue(expected)

    const result = await generateProfileToolpath(SAMPLE_PROFILE_INPUT)

    expect(mockGenerateProfileToolpath).toHaveBeenCalledWith(SAMPLE_PROFILE_INPUT)
    expect(result).toBe(expected)
  })
})

describe('emitGrblGcode', () => {
  it('forwards toolpath, tool, and stock to the emitter', async () => {
    const toolpath: ToolpathOutput = [{ kind: 'rapid', to: [0, 0, 5] }]
    mockEmitGrblGcode.mockReturnValue('G21\nG0 X0 Y0 Z5\n')

    const program = await emitGrblGcode(toolpath, SAMPLE_TOOL, SAMPLE_STOCK)

    expect(mockEmitGrblGcode).toHaveBeenCalledWith(toolpath, SAMPLE_TOOL, SAMPLE_STOCK)
    expect(program).toBe('G21\nG0 X0 Y0 Z5\n')
  })
})

describe('smoke: round-trip shape against TS types', () => {
  it('parseSvg returns a value structurally assignable to ParseSvgResult', async () => {
    // Simulate exactly what wasm hands back (the bridge does not transform it).
    const fromWasm = {
      paths: [
        {
          points: [
            { x: 0, y: 0 },
            { x: 1, y: 0 },
          ],
          closed: false,
        },
      ],
      warnings: [{ line: null, message: 'parsed' }],
    }
    mockParseSvg.mockReturnValue(fromWasm)

    const result: ParseSvgResult = await parseSvg(new Uint8Array())

    // Structural assertions matching the TS type contract.
    expect(Array.isArray(result.paths)).toBe(true)
    expect(Array.isArray(result.warnings)).toBe(true)
    expect(result.paths[0]).toMatchObject({
      points: expect.any(Array),
      closed: expect.any(Boolean),
    })
    expect(result.paths[0].points[0]).toMatchObject({
      x: expect.any(Number),
      y: expect.any(Number),
    })
    expect(result.warnings[0]).toMatchObject({
      line: null,
      message: expect.any(String),
    })
  })
})
