import type { LineGeometryData } from '../api/types'
import { decimateToolpath } from './decimateToolpath'

/** Build a LineGeometryData with `numPoints` points (must be even for valid segments). */
function makeData(numPoints: number): LineGeometryData {
  return {
    positions: Array.from({ length: numPoints * 3 }, (_, i) => i),
    colours: Array.from({ length: numPoints * 3 }, (_, i) => i),
    types: Array.from({ length: numPoints }, (_, i) => i),
  }
}

function pointCount(data: LineGeometryData): number {
  return data.positions.length / 3
}

describe('decimateToolpath', () => {
  const MAX = 4

  it('returns the same object for empty input', () => {
    const empty: LineGeometryData = { positions: [], colours: [], types: [] }
    expect(decimateToolpath(empty, MAX)).toBe(empty)
  })

  it('returns the same object when input has exactly maxPoints points', () => {
    const data = makeData(MAX)
    expect(decimateToolpath(data, MAX)).toBe(data)
  })

  it('returns the same object when input has fewer than maxPoints points', () => {
    const data = makeData(MAX - 2)
    expect(decimateToolpath(data, MAX)).toBe(data)
  })

  it('returns unchanged for a single point', () => {
    const data = makeData(1)
    expect(decimateToolpath(data, MAX)).toBe(data)
  })

  it('decimates 2×maxPoints input: result ≤ maxPoints+2, first and last points preserved', () => {
    const n = MAX * 2
    const data = makeData(n)
    const result = decimateToolpath(data, MAX)

    // Output is smaller than input
    expect(pointCount(result)).toBeLessThan(n)
    // Output is within tolerance of maxPoints (force-include of last segment adds ≤ 2 points)
    expect(pointCount(result)).toBeLessThanOrEqual(MAX + 2)

    // First point preserved
    expect(result.positions.slice(0, 3)).toEqual(data.positions.slice(0, 3))
    // Last point preserved
    expect(result.positions.slice(-3)).toEqual(data.positions.slice(-3))
  })

  it('non-exact divisor: first and last points preserved, output ≤ maxPoints+2', () => {
    const n = MAX * 2 + 2 // 10 points — not an exact multiple of MAX
    const data = makeData(n)
    const result = decimateToolpath(data, MAX)

    expect(pointCount(result)).toBeLessThan(n)
    expect(pointCount(result)).toBeLessThanOrEqual(MAX + 2)

    expect(result.positions.slice(0, 3)).toEqual(data.positions.slice(0, 3))
    expect(result.positions.slice(-3)).toEqual(data.positions.slice(-3))
  })

  it('positions, colours, and types arrays stay aligned after decimation', () => {
    const data = makeData(MAX * 3)
    const result = decimateToolpath(data, MAX)

    const pts = result.positions.length / 3
    expect(result.colours.length / 3).toBe(pts)
    expect(result.types.length).toBe(pts)
  })

  it('output point count is always even (segment-pair preservation)', () => {
    const data = makeData(MAX * 4) // 4×maxPoints input
    const result = decimateToolpath(data, MAX)

    expect(pointCount(result) % 2).toBe(0)
  })
})
