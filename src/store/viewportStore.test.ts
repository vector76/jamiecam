import { useViewportStore } from './viewportStore'
import type { DisplayMode } from './viewportStore'
import type { FaceDescriptor, MeshData } from '../api/types'
import type { SimPoint } from '../viewport/simulationPoints'

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
    simulationMeshData: null,
    orbitTarget: [0, 0, 0],
    zoom: 1,
    displayMode: 'shaded',
    projectionMode: 'perspective',
    selectionMode: false,
    hoveredFaceIdx: null,
    selectedFaceFingerprints: [],
    faceDescriptors: [],
    simulationActive: false,
    simulationPaused: false,
    simulationProgress: 0,
    simulationPlaybackSpeed: 10.0,
    simulationPoints: null,
    measurementMode: 'off',
    measurementPoints: [],
    measurements: [],
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

  it('starts with displayMode shaded', () => {
    expect(useViewportStore.getState().displayMode).toBe('shaded')
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

describe('viewportStore — simulationMeshData', () => {
  it('starts as null', () => {
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('setSimulationMeshData stores the mesh', () => {
    useViewportStore.getState().setSimulationMeshData(MESH)
    expect(useViewportStore.getState().simulationMeshData).toEqual(MESH)
  })

  it('setSimulationMeshData with null clears the simulation mesh', () => {
    useViewportStore.getState().setSimulationMeshData(MESH)
    useViewportStore.getState().setSimulationMeshData(null)
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('clearSimulationMesh sets simulationMeshData to null', () => {
    useViewportStore.getState().setSimulationMeshData(MESH)
    useViewportStore.getState().clearSimulationMesh()
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('clearSimulationMesh does not affect meshData', () => {
    useViewportStore.getState().setMeshData(MESH)
    useViewportStore.getState().setSimulationMeshData(MESH)
    useViewportStore.getState().clearSimulationMesh()
    expect(useViewportStore.getState().meshData).toEqual(MESH)
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
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

describe('viewportStore — projectionMode', () => {
  it('starts with perspective', () => {
    expect(useViewportStore.getState().projectionMode).toBe('perspective')
  })

  it('setProjectionMode sets orthographic', () => {
    useViewportStore.getState().setProjectionMode('orthographic')
    expect(useViewportStore.getState().projectionMode).toBe('orthographic')
  })

  it('setProjectionMode can toggle back to perspective', () => {
    useViewportStore.getState().setProjectionMode('orthographic')
    useViewportStore.getState().setProjectionMode('perspective')
    expect(useViewportStore.getState().projectionMode).toBe('perspective')
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

describe('viewportStore — setDisplayMode', () => {
  const modes: DisplayMode[] = ['shaded', 'shaded-edges', 'wireframe', 'transparent']

  it('starts with shaded', () => {
    expect(useViewportStore.getState().displayMode).toBe('shaded')
  })

  for (const mode of modes) {
    it(`setDisplayMode('${mode}') stores the mode`, () => {
      useViewportStore.getState().setDisplayMode(mode)
      expect(useViewportStore.getState().displayMode).toBe(mode)
    })
  }

  it('can switch between modes', () => {
    useViewportStore.getState().setDisplayMode('wireframe')
    useViewportStore.getState().setDisplayMode('transparent')
    expect(useViewportStore.getState().displayMode).toBe('transparent')
  })
})

const SIM_POINTS: SimPoint[] = [
  { x: 0, y: 0, z: 0, moveType: 0 },
  { x: 1, y: 0, z: 0, moveType: 1 },
  { x: 2, y: 1, z: 0, moveType: 1 },
]

describe('viewportStore — simulation initial state', () => {
  it('starts with simulationActive false', () => {
    expect(useViewportStore.getState().simulationActive).toBe(false)
  })

  it('starts with simulationPaused false', () => {
    expect(useViewportStore.getState().simulationPaused).toBe(false)
  })

  it('starts with simulationProgress 0', () => {
    expect(useViewportStore.getState().simulationProgress).toBe(0)
  })

  it('starts with simulationPlaybackSpeed 10.0', () => {
    expect(useViewportStore.getState().simulationPlaybackSpeed).toBe(10.0)
  })

  it('starts with simulationPoints null', () => {
    expect(useViewportStore.getState().simulationPoints).toBeNull()
  })
})

describe('viewportStore — startSimulation', () => {
  it('sets simulationActive true', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    expect(useViewportStore.getState().simulationActive).toBe(true)
  })

  it('stores the provided points', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    expect(useViewportStore.getState().simulationPoints).toEqual(SIM_POINTS)
  })

  it('resets simulationProgress to 0', () => {
    useViewportStore.setState({ simulationProgress: 0.5 })
    useViewportStore.getState().startSimulation(SIM_POINTS)
    expect(useViewportStore.getState().simulationProgress).toBe(0)
  })

  it('clears simulationPaused', () => {
    useViewportStore.setState({ simulationPaused: true })
    useViewportStore.getState().startSimulation(SIM_POINTS)
    expect(useViewportStore.getState().simulationPaused).toBe(false)
  })
})

describe('viewportStore — pauseSimulation / resumeSimulation', () => {
  it('pauseSimulation sets simulationPaused true', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().pauseSimulation()
    expect(useViewportStore.getState().simulationPaused).toBe(true)
  })

  it('pauseSimulation leaves simulationActive unchanged', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().pauseSimulation()
    expect(useViewportStore.getState().simulationActive).toBe(true)
  })

  it('resumeSimulation sets simulationPaused false', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().pauseSimulation()
    useViewportStore.getState().resumeSimulation()
    expect(useViewportStore.getState().simulationPaused).toBe(false)
  })

  it('resumeSimulation leaves simulationActive unchanged', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().pauseSimulation()
    useViewportStore.getState().resumeSimulation()
    expect(useViewportStore.getState().simulationActive).toBe(true)
  })
})

describe('viewportStore — stopSimulation', () => {
  it('sets simulationActive false', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().stopSimulation()
    expect(useViewportStore.getState().simulationActive).toBe(false)
  })

  it('sets simulationPaused false', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().pauseSimulation()
    useViewportStore.getState().stopSimulation()
    expect(useViewportStore.getState().simulationPaused).toBe(false)
  })

  it('resets simulationProgress to 0', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().setSimulationProgress(0.8)
    useViewportStore.getState().stopSimulation()
    expect(useViewportStore.getState().simulationProgress).toBe(0)
  })

  it('clears simulationPoints to null', () => {
    useViewportStore.getState().startSimulation(SIM_POINTS)
    useViewportStore.getState().stopSimulation()
    expect(useViewportStore.getState().simulationPoints).toBeNull()
  })
})

describe('viewportStore — setSimulationProgress', () => {
  it('updates the progress', () => {
    useViewportStore.getState().setSimulationProgress(0.7)
    expect(useViewportStore.getState().simulationProgress).toBe(0.7)
  })

  it('can be set to 0', () => {
    useViewportStore.getState().setSimulationProgress(0.3)
    useViewportStore.getState().setSimulationProgress(0)
    expect(useViewportStore.getState().simulationProgress).toBe(0)
  })
})

describe('viewportStore — setSimulationPlaybackSpeed', () => {
  it('updates the playback speed', () => {
    useViewportStore.getState().setSimulationPlaybackSpeed(5.0)
    expect(useViewportStore.getState().simulationPlaybackSpeed).toBe(5.0)
  })

  it('replaces the previous speed', () => {
    useViewportStore.getState().setSimulationPlaybackSpeed(2.0)
    useViewportStore.getState().setSimulationPlaybackSpeed(20.0)
    expect(useViewportStore.getState().simulationPlaybackSpeed).toBe(20.0)
  })
})

describe('viewportStore — measurement initial state', () => {
  it('starts with measurementMode off', () => {
    expect(useViewportStore.getState().measurementMode).toBe('off')
  })

  it('starts with empty measurementPoints', () => {
    expect(useViewportStore.getState().measurementPoints).toEqual([])
  })

  it('starts with empty measurements', () => {
    expect(useViewportStore.getState().measurements).toEqual([])
  })
})

describe('viewportStore — addMeasurementPoint (distance mode)', () => {
  beforeEach(() => {
    useViewportStore.getState().setMeasurementMode('distance')
  })

  it('first point does not complete a measurement', () => {
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    const state = useViewportStore.getState()
    expect(state.measurements).toHaveLength(0)
    expect(state.measurementPoints).toEqual([[0, 0, 0]])
  })

  it('second point completes measurement and resets measurementPoints', () => {
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    const state = useViewportStore.getState()
    expect(state.measurements).toHaveLength(1)
    expect(state.measurementPoints).toEqual([])
  })

  it('completed distance measurement stores correct value', () => {
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    const m = useViewportStore.getState().measurements[0]
    expect(m.value).toBeCloseTo(5, 5)
  })

  it('completed distance measurement label is formatted as mm', () => {
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    const m = useViewportStore.getState().measurements[0]
    expect(m.label).toBe('5.0 mm')
  })

  it('completed distance measurement anchor is midpoint', () => {
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([4, 0, 0])
    const m = useViewportStore.getState().measurements[0]
    expect(m.anchor).toEqual([2, 0, 0])
  })
})

describe('viewportStore — addMeasurementPoint (angle mode)', () => {
  beforeEach(() => {
    useViewportStore.getState().setMeasurementMode('angle')
  })

  it('first point does not complete a measurement', () => {
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    expect(useViewportStore.getState().measurements).toHaveLength(0)
    expect(useViewportStore.getState().measurementPoints).toHaveLength(1)
  })

  it('second point does not complete a measurement', () => {
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    expect(useViewportStore.getState().measurements).toHaveLength(0)
    expect(useViewportStore.getState().measurementPoints).toHaveLength(2)
  })

  it('third point completes measurement with correct angle value', () => {
    // 90° angle: p1=(1,0,0), vertex=(0,0,0), p3=(0,1,0)
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 1, 0])
    const state = useViewportStore.getState()
    expect(state.measurements).toHaveLength(1)
    expect(state.measurements[0].value).toBeCloseTo(90, 5)
    expect(state.measurements[0].label).toBe('90.0°')
    expect(state.measurementPoints).toEqual([])
  })

  it('completed angle measurement anchor is vertex', () => {
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 1, 0])
    const m = useViewportStore.getState().measurements[0]
    expect(m.anchor).toEqual([0, 0, 0])
  })
})

describe('viewportStore — clearMeasurements', () => {
  it('resets measurements and measurementPoints to empty arrays', () => {
    useViewportStore.getState().setMeasurementMode('distance')
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    useViewportStore.getState().clearMeasurements()
    const state = useViewportStore.getState()
    expect(state.measurements).toEqual([])
    expect(state.measurementPoints).toEqual([])
  })
})

describe('viewportStore — removeMeasurement', () => {
  it('removes the first entry and preserves remaining', () => {
    useViewportStore.getState().setMeasurementMode('distance')
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([0, 0, 5])
    const before = useViewportStore.getState().measurements
    expect(before).toHaveLength(2)
    useViewportStore.getState().removeMeasurement(0)
    const after = useViewportStore.getState().measurements
    expect(after).toHaveLength(1)
    expect(after[0]).toEqual(before[1])
  })
})

describe('viewportStore — setMeasurementMode', () => {
  it('resets measurementPoints to [] but keeps completed measurements', () => {
    useViewportStore.getState().setMeasurementMode('distance')
    useViewportStore.getState().addMeasurementPoint([0, 0, 0])
    useViewportStore.getState().addMeasurementPoint([3, 4, 0])
    // Start a new incomplete measurement
    useViewportStore.getState().addMeasurementPoint([1, 0, 0])
    expect(useViewportStore.getState().measurementPoints).toHaveLength(1)
    useViewportStore.getState().setMeasurementMode('angle')
    const state = useViewportStore.getState()
    expect(state.measurementPoints).toEqual([])
    expect(state.measurements).toHaveLength(1)
    expect(state.measurementMode).toBe('angle')
  })
})
