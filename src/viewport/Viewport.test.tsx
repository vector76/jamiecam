/**
 * Tests for the Viewport React component.
 *
 * SceneManager is mocked so no real WebGL context is required.
 * createAxisTriad is kept real (it's pure Three.js geometry).
 */

import { render, act } from '@testing-library/react'
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
    dispose: vi.fn(),
    frameModel: vi.fn(),
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
  return last.value as unknown as { scene: THREE.Scene; dispose: ReturnType<typeof vi.fn>; frameModel: ReturnType<typeof vi.fn> }
}

const QUAD_MESH: MeshData = {
  vertices: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0],
  normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2, 0, 2, 3],
}

// ── Setup / teardown ──────────────────────────────────────────────────────────

beforeEach(() => {
  vi.mocked(SceneManager).mockClear()
  // Reset viewport store to empty state before each test.
  useViewportStore.setState({ meshData: null, orbitTarget: [0, 0, 0], zoom: 1 })
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
