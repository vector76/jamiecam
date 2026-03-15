import type { LineGeometryData } from '../api/types'

export const LOD_MAX_DISPLAY_POINTS = 50_000
export const LOD_THRESHOLDS = {
  FULL: 1.5,    // camera distance < 1.5× bounding radius → full res
  HALF: 3.0,    // < 3.0× → half
  QUARTER: 6.0, // < 6.0× → quarter
  // else: eighth
} as const

export function decimateToolpath(
  data: LineGeometryData,
  maxPoints: number,
): LineGeometryData {
  const n = data.positions.length / 3
  if (n === 0 || n <= maxPoints) {
    return data
  }

  const nSegs = n / 2
  const segStep = Math.ceil(nSegs / (maxPoints / 2))

  const positions: number[] = []
  const colours: number[] = []
  const types: number[] = []
  let lastSampledSeg = -1

  for (let s = 0; s < nSegs; s += segStep) {
    const p0 = s * 2
    const p1 = s * 2 + 1
    positions.push(
      data.positions[p0 * 3], data.positions[p0 * 3 + 1], data.positions[p0 * 3 + 2],
      data.positions[p1 * 3], data.positions[p1 * 3 + 1], data.positions[p1 * 3 + 2],
    )
    colours.push(
      data.colours[p0 * 3], data.colours[p0 * 3 + 1], data.colours[p0 * 3 + 2],
      data.colours[p1 * 3], data.colours[p1 * 3 + 1], data.colours[p1 * 3 + 2],
    )
    types.push(data.types[p0], data.types[p1])
    lastSampledSeg = s
  }

  // Force-include the last segment (indices n-2 and n-1) if not already sampled
  const lastSeg = nSegs - 1
  if (lastSampledSeg !== lastSeg) {
    const p0 = lastSeg * 2       // = n - 2
    const p1 = lastSeg * 2 + 1   // = n - 1
    positions.push(
      data.positions[p0 * 3], data.positions[p0 * 3 + 1], data.positions[p0 * 3 + 2],
      data.positions[p1 * 3], data.positions[p1 * 3 + 1], data.positions[p1 * 3 + 2],
    )
    colours.push(
      data.colours[p0 * 3], data.colours[p0 * 3 + 1], data.colours[p0 * 3 + 2],
      data.colours[p1 * 3], data.colours[p1 * 3 + 1], data.colours[p1 * 3 + 2],
    )
    types.push(data.types[p0], data.types[p1])
  }

  return { positions, colours, types }
}
