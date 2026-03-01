import * as THREE from 'three'
import type { LineGeometryData } from '../api/types'
import { buildToolpathLines } from './toolpathLines'

/** Two line segments (4 vertices). */
const TWO_SEGMENTS: LineGeometryData = {
  // Segment 0: (0,0,0) → (1,0,0); Segment 1: (1,0,0) → (1,1,0)
  positions: [
    0, 0, 0,
    1, 0, 0,
    1, 0, 0,
    1, 1, 0,
  ],
  colours: [
    1, 0, 0,
    1, 0, 0,
    0, 1, 0,
    0, 1, 0,
  ],
  types: [0, 1],
}

describe('buildToolpathLines', () => {
  it('returns null for null input', () => {
    expect(buildToolpathLines(null)).toBeNull()
  })

  it('returns null for empty positions', () => {
    const empty: LineGeometryData = { positions: [], colours: [], types: [] }
    expect(buildToolpathLines(empty)).toBeNull()
  })

  it('returns a THREE.LineSegments instance', () => {
    const result = buildToolpathLines(TWO_SEGMENTS)
    expect(result).toBeInstanceOf(THREE.LineSegments)
  })

  it('position attribute count equals 4 for 2 segments', () => {
    const result = buildToolpathLines(TWO_SEGMENTS)!
    expect(result.geometry.attributes.position.count).toBe(4)
  })

  it('color attribute count equals 4 for 2 segments', () => {
    const result = buildToolpathLines(TWO_SEGMENTS)!
    expect(result.geometry.attributes.color.count).toBe(4)
  })

  it('material has vertexColors set to true', () => {
    const result = buildToolpathLines(TWO_SEGMENTS)!
    expect((result.material as THREE.LineBasicMaterial).vertexColors).toBe(true)
  })
})
