/**
 * Tests for the Viewport React component.
 *
 * SceneManager is mocked so no real WebGL context is required.
 * createAxisTriad is kept real (it's pure Three.js geometry).
 */

import { render, act, fireEvent, screen } from '@testing-library/react'
import * as THREE from 'three'
import { Viewport } from './Viewport'
import { useViewportStore } from '../store/viewportStore'
import type { MeshData } from '../api/types'

// ── Global stubs ─────────────────────────────────────────────────────────────

vi.stubGlobal('requestAnimationFrame', vi.fn(() => 1))
vi.stubGlobal('cancelAnimationFrame', vi.fn())

class MockResizeObserver {
  observe = vi.fn()
  disconnect = vi.fn()
  unobserve = vi.fn()
}
vi.stubGlobal('ResizeObserver', MockResizeObserver)

// ── Mock SceneManager ─────────────────────────────────────────────────────────
// Provide a real THREE.Scene so scene.add() / scene.remove() behave correctly.

vi.mock('./scene', () => ({
  SceneManager: vi.fn().mockImplementation(() => ({
    scene: new THREE.Scene(),
    camera: new THREE.PerspectiveCamera(),
    dispose: vi.fn(),
    frameModel: vi.fn(),
    setToolpathData: vi.fn(),
    setOrbitEnabled: vi.fn(),
    setModelMesh: vi.fn(),
    setDisplayMode: vi.fn(),
    snapTop: vi.fn(),
    snapFront: vi.fn(),
    snapRight: vi.fn(),
    snapIsometric: vi.fn(),
    toggleProjection: vi.fn(),
    getProjectionMode: vi.fn().mockReturnValue('perspective'),
    updateMeasurementLabels: vi.fn(),
    updateMeasurementPoints: vi.fn(),
    getActiveCamera: vi.fn().mockReturnValue(new THREE.PerspectiveCamera()),
    getModelMesh: vi.fn().mockReturnValue(null),
  })),
}))

// Import after mocking so we get the mocked constructor.
const { SceneManager } = await import('./scene')

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Latest SceneManager instance created by the mock. */
function latestMgr() {
  const results = vi.mocked(SceneManager).mock.results
  const last = results.at(-1)
  if (!last || last.type !== 'return') throw new Error('SceneManager not yet constructed')
  return last.value as unknown as {
    scene: THREE.Scene
    dispose: ReturnType<typeof vi.fn>
    frameModel: ReturnType<typeof vi.fn>
    setToolpathData: ReturnType<typeof vi.fn>
    setModelMesh: ReturnType<typeof vi.fn>
    setDisplayMode: ReturnType<typeof vi.fn>
    snapTop: ReturnType<typeof vi.fn>
    snapFront: ReturnType<typeof vi.fn>
    snapRight: ReturnType<typeof vi.fn>
    snapIsometric: ReturnType<typeof vi.fn>
    toggleProjection: ReturnType<typeof vi.fn>
    getProjectionMode: ReturnType<typeof vi.fn>
  }
}

const QUAD_MESH: MeshData = {
  vertices: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0],
  normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2, 0, 2, 3],
  faceGroups: [],
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  vi.mocked(SceneManager).mockClear()
  // Reset viewport store to empty state before each test.
  useViewportStore.setState({
    meshData: null,
    orbitTarget: [0, 0, 0],
    zoom: 1,
    selectionMode: false,
    hoveredFaceIdx: null,
    selectedFaceFingerprints: [],
    faceDescriptors: [],
    projectionMode: 'perspective',
    displayMode: 'shaded',
  })
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Viewport — mount / unmount', () => {
  it('renders a div element', () => {
    const { container } = render(<Viewport />)
    expect(container.firstChild).toBeInstanceOf(HTMLDivElement)
  })

  it('creates a SceneManager on mount', () => {
    render(<Viewport />)
    expect(vi.mocked(SceneManager)).toHaveBeenCalled()
  })

  it('adds the axis triad to the scene on mount', () => {
    render(<Viewport />)
    const { scene } = latestMgr()
    const triad = scene.children.find((c) => c.name === 'AxisTriad')
    expect(triad).toBeDefined()
  })

  it('disposes the SceneManager on unmount', () => {
    const { unmount } = render(<Viewport />)
    const { dispose } = latestMgr()
    unmount()
    expect(dispose).toHaveBeenCalled()
  })
})

describe('Viewport — mesh updates', () => {
  it('adds a ModelGroup to the scene when meshData is set', async () => {
    render(<Viewport />)
    const { scene } = latestMgr()

    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })

    const group = scene.children.find((c) => c.name === 'ModelGroup')
    expect(group).toBeDefined()
  })

  it('calls frameModel when meshData is set', async () => {
    render(<Viewport />)
    const { frameModel } = latestMgr()

    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })

    expect(frameModel).toHaveBeenCalledWith(expect.any(THREE.Sphere))
  })

  it('replaces the ModelGroup when meshData changes', async () => {
    render(<Viewport />)
    const { scene } = latestMgr()

    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })

    const firstGroup = scene.children.find((c) => c.name === 'ModelGroup')

    const secondMesh: MeshData = {
      vertices: [0, 0, 0, 2, 0, 0, 2, 2, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2],
      faceGroups: [],
    }

    await act(async () => {
      useViewportStore.getState().setMeshData(secondMesh)
    })

    const secondGroup = scene.children.find((c) => c.name === 'ModelGroup')
    // A new group object should have replaced the first.
    expect(secondGroup).toBeDefined()
    expect(secondGroup).not.toBe(firstGroup)
  })

  it('removes the ModelGroup when meshData is cleared', async () => {
    render(<Viewport />)
    const { scene } = latestMgr()

    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })

    await act(async () => {
      useViewportStore.getState().setMeshData(null)
    })

    const group = scene.children.find((c) => c.name === 'ModelGroup')
    expect(group).toBeUndefined()
  })
})

describe('Viewport — face selection mode', () => {
  it('entering selection mode does not throw', async () => {
    const { container } = render(<Viewport />)

    await act(async () => {
      useViewportStore.getState().setSelectionMode(true)
    })

    expect(container.firstChild).toBeInstanceOf(HTMLDivElement)
  })
})

describe('Viewport — keyboard shortcuts', () => {
  function fireKey(key: string) {
    window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }))
  }

  it('T key calls snapTop', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    fireKey('t')
    expect(mgr.snapTop).toHaveBeenCalledOnce()
  })

  it('F key calls snapFront', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    fireKey('f')
    expect(mgr.snapFront).toHaveBeenCalledOnce()
  })

  it('R key calls snapRight', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    fireKey('r')
    expect(mgr.snapRight).toHaveBeenCalledOnce()
  })

  it('I key calls snapIsometric', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    fireKey('i')
    expect(mgr.snapIsometric).toHaveBeenCalledOnce()
  })

  it('uppercase T key also calls snapTop', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    fireKey('T')
    expect(mgr.snapTop).toHaveBeenCalledOnce()
  })

  it('ignores keydown when an INPUT is focused', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()
    fireKey('t')
    expect(mgr.snapTop).not.toHaveBeenCalled()
    document.body.removeChild(input)
  })

  it('ignores keydown when a TEXTAREA is focused', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    const ta = document.createElement('textarea')
    document.body.appendChild(ta)
    ta.focus()
    fireKey('f')
    expect(mgr.snapFront).not.toHaveBeenCalled()
    document.body.removeChild(ta)
  })

  it('removes keydown listener on unmount', () => {
    const removeSpy = vi.spyOn(window, 'removeEventListener')
    const { unmount } = render(<Viewport />)
    unmount()
    const keydownRemovals = removeSpy.mock.calls.filter(([type]) => type === 'keydown')
    expect(keydownRemovals.length).toBeGreaterThan(0)
    removeSpy.mockRestore()
  })

  it('P key toggles projectionMode in the store from perspective to orthographic', async () => {
    render(<Viewport />)
    await act(async () => { fireKey('p') })
    expect(useViewportStore.getState().projectionMode).toBe('orthographic')
  })

  it('P key toggles projectionMode back to perspective on second press', async () => {
    render(<Viewport />)
    await act(async () => { fireKey('p') })
    await act(async () => { fireKey('p') })
    expect(useViewportStore.getState().projectionMode).toBe('perspective')
  })
})

describe('Viewport — toolbar button', () => {
  it('renders a button labelled Perspective by default', () => {
    render(<Viewport />)
    expect(screen.getByRole('button', { name: 'Perspective' })).toBeInTheDocument()
  })

  it('clicking the button toggles projectionMode to orthographic', async () => {
    render(<Viewport />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Perspective' }))
    })
    expect(useViewportStore.getState().projectionMode).toBe('orthographic')
  })

  it('button label changes to Orthographic after toggle', async () => {
    render(<Viewport />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Perspective' }))
    })
    expect(screen.getByRole('button', { name: 'Orthographic' })).toBeInTheDocument()
  })
})

describe('Viewport — projection mode sync', () => {
  it('does not call toggleProjection on initial mount when both start as perspective', () => {
    render(<Viewport />)
    const mgr = latestMgr()
    expect(mgr.toggleProjection).not.toHaveBeenCalled()
  })

  it('calls toggleProjection when projectionMode changes to orthographic', async () => {
    render(<Viewport />)
    const mgr = latestMgr()
    // Mock returns 'perspective', store changes to 'orthographic' → mismatch → toggle called.
    await act(async () => {
      useViewportStore.getState().setProjectionMode('orthographic')
    })
    expect(mgr.toggleProjection).toHaveBeenCalledOnce()
  })

  it('does not call toggleProjection when store and manager already agree', async () => {
    render(<Viewport />)
    const mgr = latestMgr()
    // getProjectionMode returns 'perspective'; store is already 'perspective'.
    await act(async () => {
      useViewportStore.getState().setProjectionMode('perspective')
    })
    expect(mgr.toggleProjection).not.toHaveBeenCalled()
  })
})

describe('Viewport — display mode', () => {
  it('renders a select with default value shaded', () => {
    render(<Viewport />)
    const select = screen.getByRole('combobox', { name: 'Display mode' }) as HTMLSelectElement
    expect(select.value).toBe('shaded')
  })

  it('renders all four display mode options', () => {
    render(<Viewport />)
    const select = screen.getByRole('combobox', { name: 'Display mode' })
    expect(select).toContainElement(screen.getByRole('option', { name: 'Shaded' }))
    expect(select).toContainElement(screen.getByRole('option', { name: 'Shaded + Edges' }))
    expect(select).toContainElement(screen.getByRole('option', { name: 'Wireframe' }))
    expect(select).toContainElement(screen.getByRole('option', { name: 'Transparent' }))
  })

  it('changing the select updates displayMode in the store', async () => {
    render(<Viewport />)
    await act(async () => {
      fireEvent.change(screen.getByRole('combobox', { name: 'Display mode' }), { target: { value: 'wireframe' } })
    })
    expect(useViewportStore.getState().displayMode).toBe('wireframe')
  })

  it('calls setDisplayMode on the manager when displayMode changes', async () => {
    render(<Viewport />)
    const mgr = latestMgr()
    await act(async () => {
      useViewportStore.getState().setDisplayMode('transparent')
    })
    expect(mgr.setDisplayMode).toHaveBeenCalledWith('transparent')
  })

  it('calls setModelMesh and setDisplayMode when mesh is loaded', async () => {
    render(<Viewport />)
    const mgr = latestMgr()
    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })
    expect(mgr.setModelMesh).toHaveBeenCalledWith(expect.any(THREE.Mesh))
    expect(mgr.setDisplayMode).toHaveBeenCalledWith('shaded')
  })

  it('calls setModelMesh(null) when mesh is cleared', async () => {
    render(<Viewport />)
    const mgr = latestMgr()
    await act(async () => {
      useViewportStore.getState().setMeshData(QUAD_MESH)
    })
    await act(async () => {
      useViewportStore.getState().setMeshData(null)
    })
    expect(mgr.setModelMesh).toHaveBeenLastCalledWith(null)
  })
})

describe('Viewport — measurement toolbar', () => {
  beforeEach(() => {
    useViewportStore.setState({ measurementMode: 'off', measurementPoints: [], measurements: [] })
  })

  it('clicking Ruler button sets measurementMode to distance', async () => {
    render(<Viewport />)
    await act(async () => {
      fireEvent.click(screen.getByTitle('Distance measurement'))
    })
    expect(useViewportStore.getState().measurementMode).toBe('distance')
  })

  it('clicking Protractor button sets measurementMode to angle', async () => {
    render(<Viewport />)
    await act(async () => {
      fireEvent.click(screen.getByTitle('Angle measurement'))
    })
    expect(useViewportStore.getState().measurementMode).toBe('angle')
  })

  it('clicking Clear calls clearMeasurements and sets mode to off', async () => {
    useViewportStore.setState({
      measurementMode: 'distance',
      measurements: [{ points: [[0, 0, 0], [1, 0, 0]], value: 1, label: '1.0 mm', anchor: [0.5, 0, 0] }],
      measurementPoints: [[1, 2, 3]],
    })
    render(<Viewport />)
    await act(async () => {
      fireEvent.click(screen.getByTitle('Clear measurements'))
    })
    expect(useViewportStore.getState().measurementMode).toBe('off')
    expect(useViewportStore.getState().measurements).toEqual([])
    expect(useViewportStore.getState().measurementPoints).toEqual([])
  })

  it('Ruler button has active class when measurementMode is distance', async () => {
    useViewportStore.setState({ measurementMode: 'distance' })
    render(<Viewport />)
    expect(screen.getByTitle('Distance measurement')).toHaveClass('active')
  })

  it('Protractor button has active class when measurementMode is angle', async () => {
    useViewportStore.setState({ measurementMode: 'angle' })
    render(<Viewport />)
    expect(screen.getByTitle('Angle measurement')).toHaveClass('active')
  })
})

describe('Viewport — measurement mode interactions', () => {
  beforeEach(() => {
    useViewportStore.setState({
      measurementMode: 'off',
      selectionMode: false,
      measurementPoints: [],
      measurements: [],
    })
  })

  it('Escape key sets measurementMode to off when in distance mode', async () => {
    useViewportStore.setState({ measurementMode: 'distance' })
    render(<Viewport />)
    await act(async () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
    })
    expect(useViewportStore.getState().measurementMode).toBe('off')
  })

  it('setting measurementMode to distance while selectionMode is true sets selectionMode to false', async () => {
    useViewportStore.setState({ selectionMode: true })
    render(<Viewport />)
    await act(async () => {
      useViewportStore.getState().setMeasurementMode('distance')
    })
    expect(useViewportStore.getState().selectionMode).toBe(false)
  })

  it('setting selectionMode to true while measurementMode is distance sets measurementMode to off', async () => {
    useViewportStore.setState({ measurementMode: 'distance' })
    render(<Viewport />)
    await act(async () => {
      useViewportStore.getState().setSelectionMode(true)
    })
    expect(useViewportStore.getState().measurementMode).toBe('off')
  })
})
