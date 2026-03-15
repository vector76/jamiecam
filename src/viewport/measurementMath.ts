export type Point3 = [number, number, number]

/** Returns the Euclidean distance between two 3D points in mm. */
export function distanceBetweenPoints(a: Point3, b: Point3): number {
  return Math.sqrt((b[0] - a[0]) ** 2 + (b[1] - a[1]) ** 2 + (b[2] - a[2]) ** 2)
}

/** Returns the interior angle in degrees at the vertex (middle point). */
export function angleBetweenThreePoints(a: Point3, vertex: Point3, b: Point3): number {
  const u: Point3 = [a[0] - vertex[0], a[1] - vertex[1], a[2] - vertex[2]]
  const v: Point3 = [b[0] - vertex[0], b[1] - vertex[1], b[2] - vertex[2]]
  const dot = u[0] * v[0] + u[1] * v[1] + u[2] * v[2]
  const magU = Math.sqrt(u[0] ** 2 + u[1] ** 2 + u[2] ** 2)
  const magV = Math.sqrt(v[0] ** 2 + v[1] ** 2 + v[2] ** 2)
  const cosAngle = Math.max(-1, Math.min(1, dot / (magU * magV)))
  return Math.acos(cosAngle) * (180 / Math.PI)
}
