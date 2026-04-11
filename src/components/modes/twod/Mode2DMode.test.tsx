/**
 * Tests for Mode2DMode.tsx — 2D Profiling mode top-level component.
 *
 * All Tauri IPC calls and child components that require browser APIs are
 * mocked. The real Zustand projectStore is used so snapshot-driven state
 * (stock, safeHeight, artworkOrigin) is verifiable.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { Mode2DMode } from './Mode2DMode'
import { useProjectStore } from '../../../store/projectStore'
import type { ProjectSnapshot } from '../../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('./Canvas2D', () => ({
  Canvas2D: () => <div data-testid="canvas-2d">Canvas2D</div>,
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

vi.mock('../../../api/twodMode', () => ({
  loadTwodFile: vi.fn(),
  getTwodCurves: vi.fn(),
  setSafeHeight: vi.fn(),
  setArtworkOrigin: vi.fn(),
}))

vi.mock('../../../api/stock', () => ({
  setStock: vi.fn(),
  getStock: vi.fn(),
}))

vi.mock('../../../api/operations', () => ({
  deleteOperation: vi.fn(),
}))

vi.mock('../../../api/tools', () => ({
  listTools: vi.fn(),
  deleteTool: vi.fn(),
}))

vi.mock('../../tools/ToolEditorList', () => ({
  ToolEditorList: () => <div data-testid="tool-editor-list">ToolEditorList</div>,
}))

vi.mock('@/components/ui/sidebar-section', () => ({
  SidebarSection: ({ title, children }: { title: string; children: React.ReactNode }) => (
    <div data-testid={`section-${title.toLowerCase().replace(/\s+/g, '-')}`}>{children}</div>
  ),
}))

vi.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))

// ── Lazy imports (after mocks are registered) ─────────────────────────────────

const twodApi = await import('../../../api/twodMode')
const stockApi = await import('../../../api/stock')
const operationsApi = await import('../../../api/operations')
const dialogApi = await import('@tauri-apps/plugin-dialog')
const toolsApi = await import('../../../api/tools')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const BASE_SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test Project',
  modifiedAt: '',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
  projectIsOpen: true,
  filePath: null,
  dirty: false,
  mode: '2d',
  safeHeight: null,
  artworkOrigin: [0, 0],
}

const MOCK_LOAD_RESULT = {
  curves: [
    { id: 'curve-1', isClosed: true, bbox: { minX: 0, minY: 0, maxX: 50, maxY: 50 } },
    { id: 'curve-2', isClosed: false, bbox: { minX: 10, minY: 10, maxX: 60, maxY: 60 } },
  ],
  curvePoints: {
    'curve-1': [[0, 0], [50, 0], [50, 50], [0, 50]],
    'curve-2': [[10, 10], [60, 60]],
  },
  unitSystem: 'mm' as const,
  warnings: [],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(twodApi.getTwodCurves).mockResolvedValue(null)
  vi.mocked(twodApi.loadTwodFile).mockResolvedValue(MOCK_LOAD_RESULT)
  vi.mocked(twodApi.setSafeHeight).mockResolvedValue(undefined)
  vi.mocked(twodApi.setArtworkOrigin).mockResolvedValue(undefined)
  vi.mocked(stockApi.setStock).mockResolvedValue(undefined)
  vi.mocked(operationsApi.deleteOperation).mockResolvedValue(undefined)
  vi.mocked(toolsApi.listTools).mockResolvedValue([])
  vi.mocked(toolsApi.deleteTool).mockResolvedValue(undefined)
  useProjectStore.setState({ snapshot: { ...BASE_SNAPSHOT }, notifications: [] })
})

// ── Helper: load a DXF file via the sidebar button ───────────────────────────

async function loadDxfViaButton(path = '/test/design.dxf', result = MOCK_LOAD_RESULT) {
  vi.mocked(dialogApi.open).mockResolvedValue(path)
  vi.mocked(twodApi.loadTwodFile).mockResolvedValue(result)
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
  })
  await waitFor(() => {
    expect(twodApi.loadTwodFile).toHaveBeenCalledWith(path, null)
  })
}

// ── Tests: initial render ─────────────────────────────────────────────────────

describe('Mode2DMode — initial render', () => {
  it('renders the Load 2D File button when no file is loaded', async () => {
    render(<Mode2DMode />)
    // waitFor lets mount effects (getTwodCurves, listTools) settle before asserting.
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /load 2d file/i })).toBeInTheDocument()
    })
  })

  it('does not show a file info line before a file is loaded', async () => {
    render(<Mode2DMode />)
    // After mount, getTwodCurves returns null → no file info displayed.
    await waitFor(() => {
      expect(screen.queryByText(/curves/i)).not.toBeInTheDocument()
    })
  })
})

// ── Tests: file loading ───────────────────────────────────────────────────────

describe('Mode2DMode — file loading', () => {
  it('shows file name and curve count after successful DXF load', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton('/path/to/design.dxf')

    await waitFor(() => {
      expect(screen.getByText(/design\.dxf/)).toBeInTheDocument()
      expect(screen.getByText(/2 curves/)).toBeInTheDocument()
    })
  })

  it('shows the SVG unit selection modal when an SVG file is picked', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue('/art/logo.svg')
    render(<Mode2DMode />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })

    await waitFor(() => {
      expect(screen.getByRole('dialog', { name: /select unit system/i })).toBeInTheDocument()
    })
  })

  it('calls loadTwodFile with the chosen unit when SVG unit is confirmed', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue('/art/logo.svg')
    render(<Mode2DMode />)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /millimeters/i })).toBeInTheDocument()
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /millimeters/i }))
    })

    await waitFor(() => {
      expect(twodApi.loadTwodFile).toHaveBeenCalledWith('/art/logo.svg', 'mm')
    })
  })
})

// ── Tests: file replacement confirmation ──────────────────────────────────────

describe('Mode2DMode — file replacement confirmation', () => {
  it('shows the confirmation dialog when a file is already loaded', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton('/first.dxf')

    // Pick a second file.
    vi.mocked(dialogApi.open).mockResolvedValue('/second.dxf')
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })

    await waitFor(() => {
      expect(
        screen.getByText(/loading a new file will clear all existing operations/i),
      ).toBeInTheDocument()
    })
  })

  it('loads the new file after confirmation is accepted', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton('/first.dxf')

    vi.mocked(dialogApi.open).mockResolvedValue('/second.dxf')
    vi.mocked(twodApi.loadTwodFile).mockResolvedValue(MOCK_LOAD_RESULT)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /continue/i })).toBeInTheDocument()
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /continue/i }))
    })

    await waitFor(() => {
      expect(twodApi.loadTwodFile).toHaveBeenCalledWith('/second.dxf', null)
    })
  })

  it('does not load the new file when confirmation is cancelled', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton('/first.dxf')

    const callCount = vi.mocked(twodApi.loadTwodFile).mock.calls.length

    vi.mocked(dialogApi.open).mockResolvedValue('/second.dxf')
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument()
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    })

    // loadTwodFile call count should not increase.
    expect(vi.mocked(twodApi.loadTwodFile).mock.calls.length).toBe(callCount)
  })
})

// ── Tests: operations preserved on failed load ────────────────────────────────

describe('Mode2DMode — operations preserved when new file load fails', () => {
  it('does not call deleteOperation when loadTwodFile rejects', async () => {
    render(<Mode2DMode />)
    // Load an initial file so curves.length > 0.
    await loadDxfViaButton('/first.dxf')

    // Give the snapshot some operations to potentially delete.
    await act(async () => {
      useProjectStore.setState({
        snapshot: {
          ...BASE_SNAPSHOT,
          operations: [
            {
              id: 'op-1',
              name: 'Op 1',
              operationType: 'profile_2d',
              enabled: true,
              needsRecalculate: false,
            },
          ],
        },
      })
    })

    // Mock the second load to fail.
    vi.mocked(twodApi.loadTwodFile).mockRejectedValue({
      kind: 'FileNotFound',
      message: 'File not found',
    })
    vi.mocked(dialogApi.open).mockResolvedValue('/second.dxf')

    // Trigger the confirmation flow.
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load 2d file/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /continue/i })).toBeInTheDocument()
    })
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /continue/i }))
    })

    await waitFor(() => {
      expect(twodApi.loadTwodFile).toHaveBeenCalled()
    })

    // Operations must not be deleted since the load failed.
    // Use waitFor to ensure all async effects have settled before asserting.
    await waitFor(() => {
      expect(operationsApi.deleteOperation).not.toHaveBeenCalled()
    })
  })
})

// ── Tests: project settings ───────────────────────────────────────────────────

describe('Mode2DMode — project settings', () => {
  it('calls setStock with updated width when X dimension input changes', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        stock: {
          type: 'box',
          origin: { x: 0, y: 0, z: 0 },
          width: 100,
          depth: 50,
          height: 20,
        },
      },
    })

    render(<Mode2DMode />)

    // Wait for the useEffect to sync stock fields from snapshot.
    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: /x dimension/i })).toHaveValue(100)
    })

    fireEvent.change(screen.getByRole('spinbutton', { name: /x dimension/i }), {
      target: { value: '150' },
    })

    await waitFor(() => {
      expect(stockApi.setStock).toHaveBeenCalledWith(
        expect.objectContaining({ width: 150, type: 'box' }),
      )
    })
  })

  it('calls setSafeHeight with the parsed value when safe height input changes', async () => {
    render(<Mode2DMode />)

    fireEvent.change(screen.getByRole('spinbutton', { name: /safe height/i }), {
      target: { value: '5' },
    })

    await waitFor(() => {
      expect(twodApi.setSafeHeight).toHaveBeenCalledWith(5)
    })
  })

  it('does not call setStock when non-numeric value is entered', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        stock: {
          type: 'box',
          origin: { x: 0, y: 0, z: 0 },
          width: 100,
          depth: 50,
          height: 20,
        },
      },
    })

    render(<Mode2DMode />)

    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: /x dimension/i })).toHaveValue(100)
    })

    fireEvent.change(screen.getByRole('spinbutton', { name: /x dimension/i }), {
      target: { value: '' },
    })

    // setStock should not be called when the field is empty / not a valid number.
    expect(stockApi.setStock).not.toHaveBeenCalled()
  })
})
