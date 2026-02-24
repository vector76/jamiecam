/**
 * Viewport control helpers — axis triad WCS indicator.
 *
 * The axis triad is a small RGB arrow cluster placed at the world-coordinate
 * origin.  It acts as a WCS (Work Coordinate System) indicator and always
 * shows the orientation of the three principal axes regardless of where the
 * camera is orbiting.
 *
 * The returned Object3D is added to the main scene by Viewport.tsx.  For
 * Phase 0 this is a world-space object at the origin; a corner-inset overlay
 * (secondary scene + scissor) can be added in a later bead.
 */

import * as THREE from 'three'

const ARROW_LENGTH = 50 // mm — 1/20th of the 1000mm grid
const HEAD_LENGTH = 10 // mm
const HEAD_WIDTH = 6 // mm

/**
 * Create an axis triad Object3D: three ArrowHelpers pointing along +X (red),
 * +Y (green), and +Z (blue), all originating at the world origin.
 */
export function createAxisTriad(): THREE.Object3D {
  const group = new THREE.Group()
  group.name = 'AxisTriad'

  const xArrow = new THREE.ArrowHelper(
    new THREE.Vector3(1, 0, 0),
    new THREE.Vector3(0, 0, 0),
    ARROW_LENGTH,
    0xff0000, // red — X axis
    HEAD_LENGTH,
    HEAD_WIDTH,
  )
  xArrow.name = 'x-axis'

  const yArrow = new THREE.ArrowHelper(
    new THREE.Vector3(0, 1, 0),
    new THREE.Vector3(0, 0, 0),
    ARROW_LENGTH,
    0x00ff00, // green — Y axis
    HEAD_LENGTH,
    HEAD_WIDTH,
  )
  yArrow.name = 'y-axis'

  const zArrow = new THREE.ArrowHelper(
    new THREE.Vector3(0, 0, 1),
    new THREE.Vector3(0, 0, 0),
    ARROW_LENGTH,
    0x0000ff, // blue — Z axis
    HEAD_LENGTH,
    HEAD_WIDTH,
  )
  zArrow.name = 'z-axis'

  group.add(xArrow, yArrow, zArrow)
  return group
}
