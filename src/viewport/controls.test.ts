import * as THREE from 'three'
import { createAxisTriad } from './controls'

describe('createAxisTriad', () => {
  it('returns a THREE.Object3D', () => {
    const triad = createAxisTriad()
    expect(triad).toBeInstanceOf(THREE.Object3D)
  })

  it('returns a group named AxisTriad', () => {
    const triad = createAxisTriad()
    expect(triad).toBeInstanceOf(THREE.Group)
    expect(triad.name).toBe('AxisTriad')
  })

  it('contains exactly three children', () => {
    const triad = createAxisTriad()
    expect(triad.children).toHaveLength(3)
  })

  it('all children are ArrowHelpers', () => {
    const triad = createAxisTriad()
    for (const child of triad.children) {
      expect(child).toBeInstanceOf(THREE.ArrowHelper)
    }
  })

  it('has an x-axis arrow', () => {
    const triad = createAxisTriad()
    const x = triad.children.find((c) => c.name === 'x-axis')
    expect(x).toBeDefined()
    expect(x).toBeInstanceOf(THREE.ArrowHelper)
  })

  it('has a y-axis arrow', () => {
    const triad = createAxisTriad()
    const y = triad.children.find((c) => c.name === 'y-axis')
    expect(y).toBeDefined()
  })

  it('has a z-axis arrow', () => {
    const triad = createAxisTriad()
    const z = triad.children.find((c) => c.name === 'z-axis')
    expect(z).toBeDefined()
  })

  it('x-axis arrow is red', () => {
    const triad = createAxisTriad()
    const x = triad.children.find((c) => c.name === 'x-axis') as THREE.ArrowHelper
    const mat = x.line.material as THREE.LineBasicMaterial
    expect(mat.color.getHex()).toBe(0xff0000)
  })

  it('y-axis arrow is green', () => {
    const triad = createAxisTriad()
    const y = triad.children.find((c) => c.name === 'y-axis') as THREE.ArrowHelper
    const mat = y.line.material as THREE.LineBasicMaterial
    expect(mat.color.getHex()).toBe(0x00ff00)
  })

  it('z-axis arrow is blue', () => {
    const triad = createAxisTriad()
    const z = triad.children.find((c) => c.name === 'z-axis') as THREE.ArrowHelper
    const mat = z.line.material as THREE.LineBasicMaterial
    expect(mat.color.getHex()).toBe(0x0000ff)
  })

  it('each call returns a new independent instance', () => {
    const t1 = createAxisTriad()
    const t2 = createAxisTriad()
    expect(t1).not.toBe(t2)
  })
})
