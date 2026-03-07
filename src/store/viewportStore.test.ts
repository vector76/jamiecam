import { useViewportStore } from './viewportStore'
import type { FaceDescriptor, MeshData } from '../api/types'

const MESH: MeshData = {
  vertices: [0, 1, 2, 3, 4, 5],
  normals: [0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2],
  faceGroups: [],
}

beforeEach(() => {
  // Reset store to initial state between tests.
  useViewportStore.setState({
    meshData: null,
    orbitTarget: [0, 0, 0],
    zoom: 1,
    displayMode: 'Shaded',
    selectionMode: false,
    hoveredFaceIdx: null,
    selectedFaceFingerprints: [],
    faceDescriptors: [],
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
    const first: MeshData = { vertices: [1], normals: [0], indices: [0], faceGroups: [] }
    const second: MeshData = { vertices: [2], normals: [1], indices: [0], faceGroups: [] }
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

describe('viewportStore — selection state initial values', () => {
  it('starts with selectionMode false', () => {
    expect(useViewportStore.getState().selectionMode).toBe(false)
  })

  it('starts with hoveredFaceIdx null', () => {
    expect(useViewportStore.getState().hoveredFaceIdx).toBeNull()
  })

  it('starts with empty selectedFaceFingerprints', () => {
    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual([])
  })

  it('starts with empty faceDescriptors', () => {
    expect(useViewportStore.getState().faceDescriptors).toEqual([])
  })
})

describe('viewportStore — setSelectionMode', () => {
  it('sets selectionMode to true', () => {
    useViewportStore.getState().setSelectionMode(true)
    expect(useViewportStore.getState().selectionMode).toBe(true)
  })

  it('setting false clears hoveredFaceIdx and faceDescriptors but preserves selectedFaceFingerprints', () => {
    const descriptor: FaceDescriptor = {
      fingerprint: 'fp-abc',
      faceIdx: 0,
      centroid: [0, 0, 0],
      normal: [0, 0, 1],
      area: 1.0,
    }
    useViewportStore.setState({
      selectionMode: true,
      hoveredFaceIdx: 2,
      selectedFaceFingerprints: ['fp-abc'],
      faceDescriptors: [descriptor],
    })
    useViewportStore.getState().setSelectionMode(false)
    const state = useViewportStore.getState()
    expect(state.selectionMode).toBe(false)
    expect(state.hoveredFaceIdx).toBeNull()
    expect(state.faceDescriptors).toEqual([])
    expect(state.selectedFaceFingerprints).toEqual(['fp-abc'])
  })
})

describe('viewportStore — toggleFaceSelection', () => {
  it('adds a fingerprint not already present', () => {
    useViewportStore.getState().toggleFaceSelection('fp-1')
    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-1'])
  })

  it('removes a fingerprint already present', () => {
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-1', 'fp-2'] })
    useViewportStore.getState().toggleFaceSelection('fp-1')
    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-2'])
  })
})

describe('viewportStore — clearFaceSelection', () => {
  it('empties the fingerprint list', () => {
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-1', 'fp-2'] })
    useViewportStore.getState().clearFaceSelection()
    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual([])
  })
})

describe('viewportStore — setFaceDescriptors', () => {
  it('stores the provided array', () => {
    const descriptors: FaceDescriptor[] = [
      { fingerprint: 'fp-1', faceIdx: 0, centroid: [1, 2, 3], normal: [0, 0, 1], area: 4.5 },
      { fingerprint: 'fp-2', faceIdx: 1, centroid: [4, 5, 6], normal: [1, 0, 0], area: 2.0 },
    ]
    useViewportStore.getState().setFaceDescriptors(descriptors)
    expect(useViewportStore.getState().faceDescriptors).toEqual(descriptors)
  })
})
