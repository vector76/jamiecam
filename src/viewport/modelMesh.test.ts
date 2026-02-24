import * as THREE from 'three'
import type { MeshData } from '../api/types'
import { buildModelMesh } from './modelMesh'

/** Minimal quad: 4 vertices, 2 triangles (6 indices). */
const QUAD_MESH: MeshData = {
  // 4 vertices forming a unit square in the XY plane
  vertices: [
    0, 0, 0, // v0
    1, 0, 0, // v1
    1, 1, 0, // v2
    0, 1, 0, // v3
  ],
  normals: [
    0, 0, 1, // n0
    0, 0, 1, // n1
    0, 0, 1, // n2
    0, 0, 1, // n3
  ],
  indices: [
    0, 1, 2, // tri 0
    0, 2, 3, // tri 1
  ],
}

describe('buildModelMesh', () => {
  it('returns a THREE.Mesh', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    expect(mesh).toBeInstanceOf(THREE.Mesh)
  })

  it('geometry has correct vertex count', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    expect(mesh.geometry.attributes.position.count).toBe(4)
  })

  it('geometry has correct index count', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    expect(mesh.geometry.index!.count).toBe(6)
  })

  it('position attribute contains the supplied vertex data', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    const pos = mesh.geometry.attributes.position as THREE.BufferAttribute
    expect(pos.getX(0)).toBeCloseTo(0)
    expect(pos.getX(1)).toBeCloseTo(1)
    expect(pos.getY(2)).toBeCloseTo(1)
    expect(pos.getY(3)).toBeCloseTo(1)
  })

  it('normal attribute is set', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    expect(mesh.geometry.attributes.normal).toBeDefined()
    expect(mesh.geometry.attributes.normal.count).toBe(4)
  })

  it('returns a non-null boundingSphere', () => {
    const { boundingSphere } = buildModelMesh(QUAD_MESH)
    expect(boundingSphere).toBeInstanceOf(THREE.Sphere)
  })

  it('bounding sphere encloses all vertices', () => {
    const { mesh, boundingSphere } = buildModelMesh(QUAD_MESH)
    const pos = mesh.geometry.attributes.position as THREE.BufferAttribute
    for (let i = 0; i < pos.count; i++) {
      const v = new THREE.Vector3(pos.getX(i), pos.getY(i), pos.getZ(i))
      expect(boundingSphere.containsPoint(v)).toBe(true)
    }
  })

  it('applies MeshStandardMaterial', () => {
    const { mesh } = buildModelMesh(QUAD_MESH)
    expect(mesh.material).toBeInstanceOf(THREE.MeshStandardMaterial)
  })

  it('handles a single triangle (3 vertices, 3 indices)', () => {
    const tri: MeshData = {
      vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2],
    }
    const { mesh } = buildModelMesh(tri)
    expect(mesh.geometry.attributes.position.count).toBe(3)
    expect(mesh.geometry.index!.count).toBe(3)
  })
})
