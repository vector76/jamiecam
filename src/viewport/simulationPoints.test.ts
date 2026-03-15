import { describe, it, expect } from 'vitest'
import {
  extractSimPoints,
  buildCumulativeDistances,
  indexAtFraction,
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

describe('indexAtFraction', () => {
  const pts = [
    { x: 0, y: 0, z: 0, moveType: 0 },
    { x: 3, y: 0, z: 0, moveType: 0 },
    { x: 8, y: 0, z: 0, moveType: 0 },
  ]
  const cd = buildCumulativeDistances(pts) // [0, 3, 8]

  it('fraction=0 → index 0', () => {
    expect(indexAtFraction(cd, 0)).toBe(0)
  })

  it('fraction=1 → last index', () => {
    expect(indexAtFraction(cd, 1)).toBe(2)
  })

  it('maps midpoint correctly on path with unequal segment lengths', () => {
    // total=8; 3/8=0.375 is closer to index 1 (dist 3) than index 0 (dist 0)
    // fraction=3/8 → target=3 → exactly index 1
    expect(indexAtFraction(cd, 3 / 8)).toBe(1)
    // fraction slightly below 3/8 → still closer to index 1
    expect(indexAtFraction(cd, 0.37)).toBe(1)
    // fraction slightly above 3/8 → target > 3, next index is 2
    expect(indexAtFraction(cd, 0.5)).toBe(1) // midpoint of [3..8] = 5.5, index 1 (dist 3) closer than index 2 (dist 8)? No: 5.5-3=2.5 vs 8-5.5=2.5, tie goes to lo
  })

  it('handles single-point path', () => {
    expect(indexAtFraction([0], 0)).toBe(0)
    expect(indexAtFraction([0], 0.5)).toBe(0)
    expect(indexAtFraction([0], 1)).toBe(0)
  })

  it('handles empty cumDist', () => {
    expect(indexAtFraction([], 0.5)).toBe(0)
  })
})
