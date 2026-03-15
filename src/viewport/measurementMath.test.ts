import { describe, it, expect } from 'vitest'
import { distanceBetweenPoints, angleBetweenThreePoints } from './measurementMath'

describe('distanceBetweenPoints', () => {
  it('returns 0 for identical points', () => {
    expect(distanceBetweenPoints([1, 2, 3], [1, 2, 3])).toBe(0)
  })

  it('returns 5 for [0,0,0] to [3,4,0]', () => {
    expect(distanceBetweenPoints([0, 0, 0], [3, 4, 0])).toBe(5)
  })

  it('returns 5 for [1,2,3] to [4,6,3]', () => {
    expect(distanceBetweenPoints([1, 2, 3], [4, 6, 3])).toBe(5)
  })
})

describe('angleBetweenThreePoints', () => {
  it('returns 90° for a right angle', () => {
    expect(angleBetweenThreePoints([1, 0, 0], [0, 0, 0], [0, 1, 0])).toBeCloseTo(90)
  })

  it('returns 180° for a straight line', () => {
    expect(angleBetweenThreePoints([-1, 0, 0], [0, 0, 0], [1, 0, 0])).toBeCloseTo(180)
  })

  it('returns 60° for an equilateral triangle vertex', () => {
    const a: [number, number, number] = [1, 0, 0]
    const vertex: [number, number, number] = [0, 0, 0]
    const b: [number, number, number] = [0.5, Math.sqrt(3) / 2, 0]
    expect(angleBetweenThreePoints(a, vertex, b)).toBeCloseTo(60)
  })
})
