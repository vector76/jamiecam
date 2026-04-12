import { describe, it, expect } from 'vitest'
import {
  extractSimPoints,
  buildCumulativeDistances,
  interpolateAtFraction,
} from './simulationPoints'
import type { LineGeometryData } from '../api/types'

describe('extractSimPoints', () => {
  it('extracts 2 points from a single segment', () => {
    const data: LineGeometryData = {
      positions: [0, 0, 0, 1, 0, 0],
      colours: [],
      types: [1],
    }
    const pts = extractSimPoints(data)
    expect(pts).toHaveLength(2)
    expect(pts[0]).toEqual({ x: 0, y: 0, z: 0, moveType: 1 })
    expect(pts[1]).toEqual({ x: 1, y: 0, z: 0, moveType: 1 })
  })

  it('de-duplicates shared endpoint between two connected segments → 3 points', () => {
    // Segment 0: (0,0,0)→(1,0,0), Segment 1: (1,0,0)→(2,0,0)
    const data: LineGeometryData = {
      positions: [0, 0, 0, 1, 0, 0, 1, 0, 0, 2, 0, 0],
      colours: [],
      types: [1, 2],
    }
    const pts = extractSimPoints(data)
    expect(pts).toHaveLength(3)
    expect(pts[0]).toEqual({ x: 0, y: 0, z: 0, moveType: 1 })
    expect(pts[1]).toEqual({ x: 1, y: 0, z: 0, moveType: 1 })
    expect(pts[2]).toEqual({ x: 2, y: 0, z: 0, moveType: 2 })
  })

  it('assigns moveType from the segment to points', () => {
    const data: LineGeometryData = {
      positions: [0, 0, 0, 5, 0, 0, 5, 0, 0, 5, 3, 0],
      colours: [],
      types: [3, 7],
    }
    const pts = extractSimPoints(data)
    expect(pts[0].moveType).toBe(3)
    expect(pts[1].moveType).toBe(3)
    expect(pts[2].moveType).toBe(7)
  })
})

describe('buildCumulativeDistances', () => {
  it('returns [0, 3, 8] for a 3-point path with known segment lengths', () => {
    // (0,0,0)→(3,0,0): distance 3; (3,0,0)→(8,0,0): distance 5 → total 8
    const pts = [
      { x: 0, y: 0, z: 0, moveType: 0 },
      { x: 3, y: 0, z: 0, moveType: 0 },
      { x: 8, y: 0, z: 0, moveType: 0 },
    ]
    const cd = buildCumulativeDistances(pts)
    expect(cd).toHaveLength(3)
    expect(cd[0]).toBe(0)
    expect(cd[1]).toBeCloseTo(3)
    expect(cd[2]).toBeCloseTo(8)
  })

  it('returns empty array for empty input', () => {
    expect(buildCumulativeDistances([])).toEqual([])
  })

  it('returns [0] for single-point input', () => {
    const pts = [{ x: 1, y: 2, z: 3, moveType: 0 }]
    expect(buildCumulativeDistances(pts)).toEqual([0])
  })
})

describe('interpolateAtFraction', () => {
  const pts = [
    { x: 0, y: 0, z: 0, moveType: 0 },
    { x: 4, y: 0, z: 0, moveType: 0 },
    { x: 4, y: 6, z: 0, moveType: 0 },
  ]
  const cd = buildCumulativeDistances(pts) // [0, 4, 10]

  it('fraction=0 returns the first point', () => {
    const p = interpolateAtFraction(pts, cd, 0)
    expect(p.x).toBeCloseTo(0)
    expect(p.y).toBeCloseTo(0)
  })

  it('fraction=1 returns the last point', () => {
    const p = interpolateAtFraction(pts, cd, 1)
    expect(p.x).toBeCloseTo(4)
    expect(p.y).toBeCloseTo(6)
  })

  it('interpolates midway along first segment', () => {
    // total = 10, first segment length = 4, midpoint of first seg at dist=2 → fraction=0.2
    const p = interpolateAtFraction(pts, cd, 0.2)
    expect(p.x).toBeCloseTo(2)
    expect(p.y).toBeCloseTo(0)
  })

  it('interpolates midway along second segment', () => {
    // At dist=7 (fraction=0.7): 3 units into second seg of length 6 → 50% along it
    const p = interpolateAtFraction(pts, cd, 0.7)
    expect(p.x).toBeCloseTo(4)
    expect(p.y).toBeCloseTo(3)
  })

  it('returns origin for empty points', () => {
    const p = interpolateAtFraction([], [], 0.5)
    expect(p.x).toBe(0)
    expect(p.y).toBe(0)
    expect(p.z).toBe(0)
  })
})
