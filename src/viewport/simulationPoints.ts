import type { LineGeometryData } from '../api/types'

export interface SimPoint {
  x: number
  y: number
  z: number
  moveType: number
}

export function extractSimPoints(data: LineGeometryData): SimPoint[] {
  const { positions, types } = data
  const segmentCount = types.length
  const points: SimPoint[] = []

  for (let seg = 0; seg < segmentCount; seg++) {
    const base = seg * 6
    const sx = positions[base]
    const sy = positions[base + 1]
    const sz = positions[base + 2]
    const ex = positions[base + 3]
    const ey = positions[base + 4]
    const ez = positions[base + 5]
    const moveType = types[seg]

    // Add start point unless it duplicates the previous point
    if (
      points.length === 0 ||
      sx !== points[points.length - 1].x ||
      sy !== points[points.length - 1].y ||
      sz !== points[points.length - 1].z
    ) {
      points.push({ x: sx, y: sy, z: sz, moveType })
    }

    // Add end point unless it duplicates the start point of this segment
    if (ex !== sx || ey !== sy || ez !== sz) {
      points.push({ x: ex, y: ey, z: ez, moveType })
    }
  }

  return points
}

export function buildCumulativeDistances(points: SimPoint[]): number[] {
  if (points.length === 0) return []
  const cumDist = [0]
  for (let i = 1; i < points.length; i++) {
    const dx = points[i].x - points[i - 1].x
    const dy = points[i].y - points[i - 1].y
    const dz = points[i].z - points[i - 1].z
    cumDist.push(cumDist[i - 1] + Math.sqrt(dx * dx + dy * dy + dz * dz))
  }
  return cumDist
}

/** Interpolated position along the toolpath at the given fraction of total distance. */
export interface InterpolatedPoint {
  x: number
  y: number
  z: number
}

export function interpolateAtFraction(
  points: SimPoint[],
  cumDist: number[],
  fraction: number,
): InterpolatedPoint {
  if (points.length === 0) return { x: 0, y: 0, z: 0 }
  if (fraction <= 0) return { x: points[0].x, y: points[0].y, z: points[0].z }
  if (fraction >= 1) {
    const last = points[points.length - 1]
    return { x: last.x, y: last.y, z: last.z }
  }

  const totalLength = cumDist[cumDist.length - 1]
  const target = totalLength * fraction

  // Binary search: find first index where cumDist[i] >= target
  let lo = 0
  let hi = cumDist.length - 1
  while (lo < hi) {
    const mid = (lo + hi) >> 1
    if (cumDist[mid] < target) lo = mid + 1
    else hi = mid
  }

  // lo is the first index with cumDist >= target
  if (lo === 0) return { x: points[0].x, y: points[0].y, z: points[0].z }

  const segStart = lo - 1
  const segLen = cumDist[lo] - cumDist[segStart]
  const t = segLen > 0 ? (target - cumDist[segStart]) / segLen : 0
  const a = points[segStart]
  const b = points[lo]

  return {
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
    z: a.z + (b.z - a.z) * t,
  }
}
