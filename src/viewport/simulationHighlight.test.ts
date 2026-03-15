import * as THREE from 'three'
import { createHighlightIndicator } from './simulationHighlight'

describe('createHighlightIndicator', () => {
  it('returns a THREE.Mesh with non-null geometry', () => {
    const indicator = createHighlightIndicator()
    expect(indicator).toBeInstanceOf(THREE.Mesh)
    expect(indicator.geometry).not.toBeNull()
  })

  it('has a non-null material', () => {
    const indicator = createHighlightIndicator()
    expect(indicator.material).not.toBeNull()
  })
})
