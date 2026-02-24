/**
 * Builds a Three.js Mesh from tessellated geometry data produced by Rust.
 *
 * The geometry data arrives as plain number arrays (from JSON IPC); these are
 * converted to typed arrays and uploaded to Three.js BufferGeometry directly,
 * with no intermediate copy.
 */

import * as THREE from 'three'
import type { MeshData } from '../api/types'

/** Material used for the primary solid model — neutral machined-metal grey. */
const SOLID_MATERIAL = new THREE.MeshStandardMaterial({
  color: 0x8a8a8a,
  roughness: 0.6,
  metalness: 0.3,
})

export interface ModelMeshResult {
  mesh: THREE.Mesh
  boundingSphere: THREE.Sphere
}

/**
 * Build a `THREE.Mesh` from `meshData` and compute its bounding sphere.
 *
 * The caller is responsible for adding the returned mesh to the scene and
 * passing `boundingSphere` to `SceneManager.frameModel()`.
 */
export function buildModelMesh(meshData: MeshData): ModelMeshResult {
  const geometry = new THREE.BufferGeometry()

  geometry.setAttribute(
    'position',
    new THREE.BufferAttribute(new Float32Array(meshData.vertices), 3),
  )
  geometry.setAttribute(
    'normal',
    new THREE.BufferAttribute(new Float32Array(meshData.normals), 3),
  )
  geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(meshData.indices), 1))

  geometry.computeBoundingSphere()
  // boundingSphere is always set after computeBoundingSphere(); use fallback for the
  // type-checker's benefit (it declares the field as Sphere | null).
  const boundingSphere = geometry.boundingSphere ?? new THREE.Sphere()

  const mesh = new THREE.Mesh(geometry, SOLID_MATERIAL)
  return { mesh, boundingSphere }
}
