/**
 * Tests for Mode3dMode.tsx — 3D Surface mode component.
 *
 * Viewport and Tauri IPC are mocked. Real Zustand stores are used so state
 * mutations are directly verifiable.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { Mode3dMode } from './Mode3dMode'
import { useViewportStore } from '../../store/viewportStore'
import { useProjectStore } from '../../store/projectStore'
import type { MeshData } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../viewport/Viewport', () => ({
  Viewport: (props: { className?: string }) => (
    <div data-testid="viewport" className={props.className}>
      Viewport
    </div>
  ),
}))

vi.mock('../../api/heightmap', () => ({
  loadHeightmap: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

vi.mock('../../lib/unsavedGuard', () => ({
  checkUnsavedChanges: vi.fn(),
}))

vi.mock('@/components/ui/sidebar-section', () => ({
  SidebarSection: ({ title, children }: { title: string; children: React.ReactNode }) => (
    <div data-testid={`section-${title.toLowerCase()}`}>{children}</div>
  ),
}))

vi.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))

const heightmapApi = await import('../../api/heightmap')
const dialogApi = await import('@tauri-apps/plugin-dialog')
const unsavedGuard = await import('../../lib/unsavedGuard')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_MESH: MeshData = {
  vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0, 1, 1, 0],
  normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2, 1, 3, 2],
  faceGroups: [{ startTriangle: 0, triangleCount: 2 }],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
  useViewportStore.setState({ meshData: null, toolpathGeometry: null, simulationMeshData: null })
})

// ── Helper ────────────────────────────────────────────────────────────────────

async function loadViaButton(path = '/test/heightmap.png', result: MeshData = MOCK_MESH) {
  vi.mocked(dialogApi.open).mockResolvedValue(path)
  vi.mocked(heightmapApi.loadHeightmap).mockResolvedValue(result)
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /open heightmap/i }))
  })
  await waitFor(() => {
    expect(heightmapApi.loadHeightmap).toHaveBeenCalledWith(path)
  })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Mode3dMode — initial render', () => {
  it('renders the viewport and the Open Heightmap button', () => {
    render(<Mode3dMode />)
    expect(screen.getByTestId('viewport')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /open heightmap/i })).toBeInTheDocument()
  })
})

describe('Mode3dMode — viewport lifecycle', () => {
  it('clears stale mesh / toolpath state on mount', () => {
    useViewportStore.setState({ meshData: MOCK_MESH })
    render(<Mode3dMode />)
    expect(useViewportStore.getState().meshData).toBeNull()
  })

  it('clears mesh on unmount', async () => {
    const { unmount } = render(<Mode3dMode />)
    await loadViaButton()
    expect(useViewportStore.getState().meshData).toEqual(MOCK_MESH)
    unmount()
    expect(useViewportStore.getState().meshData).toBeNull()
  })
})

describe('Mode3dMode — heightmap load', () => {
  it('pushes returned mesh into the viewport store on success', async () => {
    render(<Mode3dMode />)
    await loadViaButton()
    expect(useViewportStore.getState().meshData).toEqual(MOCK_MESH)
  })

  it('shows the selected filename after a successful load', async () => {
    render(<Mode3dMode />)
    await loadViaButton('/some/where/face.png')
    await waitFor(() => {
      expect(screen.getByText('face.png')).toBeInTheDocument()
    })
  })

  it('shows an inline error when the backend rejects the file', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue('/bad.png')
    vi.mocked(heightmapApi.loadHeightmap).mockRejectedValue({
      kind: 'GeometryImport',
      message: 'failed to decode image: bad PNG',
    })
    render(<Mode3dMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open heightmap/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/failed to decode image/i)
    })
    expect(useViewportStore.getState().meshData).toBeNull()
  })

  it('does nothing when the file dialog is cancelled', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue(null)
    render(<Mode3dMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open heightmap/i }))
    })
    expect(heightmapApi.loadHeightmap).not.toHaveBeenCalled()
    expect(useViewportStore.getState().meshData).toBeNull()
  })
})

describe('Mode3dMode — back button', () => {
  it('returns to selector after unsaved-changes guard', async () => {
    vi.mocked(unsavedGuard.checkUnsavedChanges).mockResolvedValue(true)
    const spy = vi.spyOn(useProjectStore.getState(), 'returnToSelector')
    render(<Mode3dMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /back/i }))
    })
    await waitFor(() => expect(spy).toHaveBeenCalled())
  })

  it('does not return when guard rejects', async () => {
    vi.mocked(unsavedGuard.checkUnsavedChanges).mockResolvedValue(false)
    const spy = vi.spyOn(useProjectStore.getState(), 'returnToSelector')
    render(<Mode3dMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /back/i }))
    })
    await waitFor(() => expect(unsavedGuard.checkUnsavedChanges).toHaveBeenCalled())
    expect(spy).not.toHaveBeenCalled()
  })
})
