import { useViewportStore } from './viewportStore'
import type { MeshData } from '../api/types'

const MESH: MeshData = {
  vertices: [0, 1, 2, 3, 4, 5],
  normals: [0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2],
}

beforeEach(() => {
  // Reset store to initial state between tests.
  useViewportStore.setState({
    meshData: null,
    orbitTarget: [0, 0, 0],
    zoom: 1,
    displayMode: 'Shaded',
  })
})

describe('viewportStore — initial state', () => {
  it('starts with null meshData', () => {
    expect(useViewportStore.getState().meshData).toBeNull()
  })

  it('starts with orbitTarget [0, 0, 0]', () => {
    expect(useViewportStore.getState().orbitTarget).toEqual([0, 0, 0])
  })

  it('starts with zoom 1', () => {
    expect(useViewportStore.getState().zoom).toBe(1)
  })

  it('starts with displayMode Shaded', () => {
    expect(useViewportStore.getState().displayMode).toBe('Shaded')
  })
})

describe('viewportStore — setMeshData', () => {
  it('stores mesh data', () => {
    useViewportStore.getState().setMeshData(MESH)
    expect(useViewportStore.getState().meshData).toEqual(MESH)
  })

  it('clears mesh data when passed null', () => {
    useViewportStore.getState().setMeshData(MESH)
    useViewportStore.getState().setMeshData(null)
    expect(useViewportStore.getState().meshData).toBeNull()
  })

  it('replaces existing mesh data', () => {
    const first: MeshData = { vertices: [1], normals: [0], indices: [0] }
    const second: MeshData = { vertices: [2], normals: [1], indices: [0] }
    useViewportStore.getState().setMeshData(first)
    useViewportStore.getState().setMeshData(second)
    expect(useViewportStore.getState().meshData).toEqual(second)
  })
})

describe('viewportStore — setOrbitTarget', () => {
  it('updates the orbit target', () => {
    useViewportStore.getState().setOrbitTarget(1, 2, 3)
    expect(useViewportStore.getState().orbitTarget).toEqual([1, 2, 3])
  })

  it('can be called multiple times with different values', () => {
    useViewportStore.getState().setOrbitTarget(1, 2, 3)
    useViewportStore.getState().setOrbitTarget(10, 20, 30)
    expect(useViewportStore.getState().orbitTarget).toEqual([10, 20, 30])
  })

  it('accepts negative coordinates', () => {
    useViewportStore.getState().setOrbitTarget(-1, -2, -3)
    expect(useViewportStore.getState().orbitTarget).toEqual([-1, -2, -3])
  })
})

describe('viewportStore — setZoom', () => {
  it('updates the zoom level', () => {
    useViewportStore.getState().setZoom(2.5)
    expect(useViewportStore.getState().zoom).toBe(2.5)
  })

  it('accepts fractional zoom values', () => {
    useViewportStore.getState().setZoom(0.25)
    expect(useViewportStore.getState().zoom).toBe(0.25)
  })

  it('replaces previous zoom value', () => {
    useViewportStore.getState().setZoom(3)
    useViewportStore.getState().setZoom(1.5)
    expect(useViewportStore.getState().zoom).toBe(1.5)
  })
})
