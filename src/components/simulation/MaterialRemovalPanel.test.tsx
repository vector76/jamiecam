import { render, screen, fireEvent, act, waitFor } from '@testing-library/react'
import { MaterialRemovalPanel } from './MaterialRemovalPanel'
import { useViewportStore } from '../../store/viewportStore'
import type { MeshData } from '../../api/types'

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('../../api/dexel', () => ({
  getSimulationMesh: vi.fn(),
}))

const { getSimulationMesh } = await import('../../api/dexel')

const MESH: MeshData = {
  vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0],
  normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2],
  faceGroups: [{ startTriangle: 0, triangleCount: 1 }],
}

function resetStores() {
  useViewportStore.setState({ simulationMeshData: null })
}

beforeEach(() => {
  resetStores()
  vi.mocked(getSimulationMesh).mockClear()
})

// ── Initial render ───────────────────────────────────────────────────────────

describe('MaterialRemovalPanel — initial state', () => {
  it('renders the Simulate button', () => {
    render(<MaterialRemovalPanel />)
    expect(screen.getByRole('button', { name: 'Simulate' })).toBeInTheDocument()
  })

  it('does not render a Demo button', () => {
    render(<MaterialRemovalPanel />)
    expect(screen.queryByRole('button', { name: 'Demo' })).not.toBeInTheDocument()
  })

  it('does not show Clear button when no simulation mesh is loaded', () => {
    render(<MaterialRemovalPanel />)
    expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
  })

  it('does not show status text when no simulation mesh is loaded', () => {
    render(<MaterialRemovalPanel />)
    expect(screen.queryByText('Showing simulated workpiece')).not.toBeInTheDocument()
  })

  it('renders the resolution select defaulting to 0.5 mm', () => {
    render(<MaterialRemovalPanel />)
    const select = screen.getByRole('combobox') as HTMLSelectElement
    expect(select.value).toBe('0.5')
  })
})

// ── Simulate button ──────────────────────────────────────────────────────────

describe('MaterialRemovalPanel — Simulate', () => {
  it('calls getSimulationMesh with the selected resolution', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(getSimulationMesh).toHaveBeenCalledWith(0.5)
  })

  it('passes the changed resolution to getSimulationMesh', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    fireEvent.change(screen.getByRole('combobox'), { target: { value: '0.1' } })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(getSimulationMesh).toHaveBeenCalledWith(0.1)
  })

  it('sets simulationMeshData in viewport store on success', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(useViewportStore.getState().simulationMeshData).toEqual(MESH)
  })

  it('shows status text and Clear button after success', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(screen.getByText('Showing simulated workpiece')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Clear' })).toBeInTheDocument()
  })

  it('disables the button while Simulating', async () => {
    let resolve: (m: MeshData) => void = () => {}
    vi.mocked(getSimulationMesh).mockReturnValue(
      new Promise<MeshData>((res) => { resolve = res }),
    )
    render(<MaterialRemovalPanel />)

    fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Simulating…' })).toBeDisabled()
    })

    await act(async () => { resolve(MESH) })
  })

  it('does not set simulationMeshData on error', async () => {
    vi.mocked(getSimulationMesh).mockRejectedValue({ kind: 'InvalidInput', message: 'no stock' })
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('re-enables button after error', async () => {
    vi.mocked(getSimulationMesh).mockRejectedValue({ kind: 'InvalidInput', message: 'no stock' })
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })

    expect(screen.getByRole('button', { name: 'Simulate' })).not.toBeDisabled()
  })
})

// ── Clear button ─────────────────────────────────────────────────────────────

describe('MaterialRemovalPanel — Clear', () => {
  it('clicking Clear sets simulationMeshData to null', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    })

    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('Clear button and status text disappear after clearing', async () => {
    vi.mocked(getSimulationMesh).mockResolvedValue(MESH)
    render(<MaterialRemovalPanel />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Simulate' }))
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 'Clear' }))
    })

    expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
    expect(screen.queryByText('Showing simulated workpiece')).not.toBeInTheDocument()
  })
})
