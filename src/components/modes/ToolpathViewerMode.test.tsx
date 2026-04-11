/**
 * Tests for ToolpathViewerMode.tsx — G-code Viewer mode component.
 *
 * All child components that depend on WebGL or Tauri APIs are mocked.
 * The real Zustand stores are used so state mutations are verifiable.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { ToolpathViewerMode } from './ToolpathViewerMode'
import { useViewportStore } from '../../store/viewportStore'
import { useProjectStore } from '../../store/projectStore'
import type { GcodeViewerLoadResult, LineGeometryData, MeshData } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../viewport/Viewport', () => ({
  Viewport: (props: { className?: string }) => (
    <div data-testid="viewport" className={props.className}>
      Viewport
    </div>
  ),
}))

vi.mock('../../api/gcodeViewer', () => ({
  loadGcodeForViewer: vi.fn(),
  simulateGcodeViewer: vi.fn(),
  getSampleGcodePath: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

vi.mock('../../lib/unsavedGuard', () => ({
  checkUnsavedChanges: vi.fn(),
}))

vi.mock('@/components/ui/sidebar-section', () => ({
  SidebarSection: ({
    title,
    children,
  }: {
    title: string
    children: React.ReactNode
  }) => <div data-testid={`section-${title.toLowerCase()}`}>{children}</div>,
}))

vi.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}))

// ── Lazy module imports (after mocks are registered) ──────────────────────────

const gcodeApi = await import('../../api/gcodeViewer')
const dialogApi = await import('@tauri-apps/plugin-dialog')
const unsavedGuard = await import('../../lib/unsavedGuard')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_LINE_GEOMETRY: LineGeometryData = {
  positions: [0, 0, 0, 1, 0, 0],
  colours: [1, 1, 1, 1, 1, 1],
  types: [1],
}

const MOCK_LOAD_RESULT: GcodeViewerLoadResult = {
  stock: {
    stockType: 'box',
    width: 100,
    depth: 100,
    height: 20,
    origin: { x: 0, y: 0, z: 0 },
  },
  tools: [{ number: 1, toolType: 'flat_endmill', diameter: 10 }],
  lineGeometry: MOCK_LINE_GEOMETRY,
  warnings: [],
}

const MOCK_MESH: MeshData = {
  vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0],
  normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
  indices: [0, 1, 2],
  faceGroups: [],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
  useViewportStore.setState({ toolpathGeometry: null, simulationMeshData: null })
})

// ── Helper: load a file via the Open File button ───────────────────────────────

async function loadFileViaButton(path = '/test/file.nc', result = MOCK_LOAD_RESULT) {
  vi.mocked(dialogApi.open).mockResolvedValue(path)
  vi.mocked(gcodeApi.loadGcodeForViewer).mockResolvedValue(result)
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /open file/i }))
  })
  await waitFor(() => {
    expect(gcodeApi.loadGcodeForViewer).toHaveBeenCalledWith(path)
  })
}

// ── Tests: viewport lifecycle ─────────────────────────────────────────────────

describe('ToolpathViewerMode — viewport lifecycle', () => {
  it('clears toolpathGeometry and simulationMeshData on mount', () => {
    useViewportStore.setState({
      toolpathGeometry: MOCK_LINE_GEOMETRY,
      simulationMeshData: MOCK_MESH,
    })

    render(<ToolpathViewerMode />)

    expect(useViewportStore.getState().toolpathGeometry).toBeNull()
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })

  it('clears viewport state on unmount', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue('/test/file.nc')
    vi.mocked(gcodeApi.loadGcodeForViewer).mockResolvedValue(MOCK_LOAD_RESULT)

    const { unmount } = render(<ToolpathViewerMode />)
    await loadFileViaButton()

    // After load, toolpathGeometry is set.
    expect(useViewportStore.getState().toolpathGeometry).toEqual(MOCK_LINE_GEOMETRY)

    unmount()

    expect(useViewportStore.getState().toolpathGeometry).toBeNull()
    expect(useViewportStore.getState().simulationMeshData).toBeNull()
  })
})

// ── Tests: initial render ─────────────────────────────────────────────────────

describe('ToolpathViewerMode — initial render', () => {
  it('renders the viewport', () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByTestId('viewport')).toBeInTheDocument()
  })

  it('renders the Open File and Load Sample buttons', () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByRole('button', { name: /open file/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /load sample/i })).toBeInTheDocument()
  })

  it('renders the Simulate button disabled when no file is loaded', () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByRole('button', { name: /simulate/i })).toBeDisabled()
  })
})

// ── Tests: file loading ───────────────────────────────────────────────────────

describe('ToolpathViewerMode — Load Sample button', () => {
  it('calls getSampleGcodePath and then loads the returned path', async () => {
    vi.mocked(gcodeApi.getSampleGcodePath).mockResolvedValue('/sample/demo-pocket.nc')
    vi.mocked(gcodeApi.loadGcodeForViewer).mockResolvedValue(MOCK_LOAD_RESULT)

    render(<ToolpathViewerMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /load sample/i }))
    })

    await waitFor(() => {
      expect(gcodeApi.getSampleGcodePath).toHaveBeenCalled()
      expect(gcodeApi.loadGcodeForViewer).toHaveBeenCalledWith('/sample/demo-pocket.nc')
    })
  })
})

describe('ToolpathViewerMode — loading a file with metadata', () => {
  it('populates stock fields from metadata and sets toolpathGeometry on the store', async () => {
    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    await waitFor(() => {
      // Width field shows metadata value.
      expect(screen.getByRole('spinbutton', { name: 'Width' })).toHaveValue(100)
      // Toolpath geometry is set on the viewport store.
      expect(useViewportStore.getState().toolpathGeometry).toEqual(MOCK_LINE_GEOMETRY)
    })
  })

  it('populates tool type and diameter fields from metadata', async () => {
    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    await waitFor(() => {
      expect(screen.getByRole('combobox', { name: 'Tool Type' })).toHaveValue('flat_endmill')
      expect(screen.getByRole('spinbutton', { name: 'Diameter' })).toHaveValue(10)
    })
  })
})

describe('ToolpathViewerMode — loading a file with no metadata', () => {
  it('leaves stock and tool fields empty, keeping Simulate disabled', async () => {
    const noMetaResult: GcodeViewerLoadResult = {
      stock: null,
      tools: [],
      lineGeometry: MOCK_LINE_GEOMETRY,
      warnings: [],
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/empty.nc', noMetaResult)

    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: 'Width' })).toHaveValue(null)
      expect(screen.getByRole('button', { name: /simulate/i })).toBeDisabled()
    })
  })
})

describe('ToolpathViewerMode — loading a second file', () => {
  it('resets overrides and repopulates from the new file metadata', async () => {
    const secondResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      stock: { ...MOCK_LOAD_RESULT.stock!, width: 200 },
    }

    render(<ToolpathViewerMode />)

    // Load first file.
    await loadFileViaButton('/first.nc', MOCK_LOAD_RESULT)
    // Override width.
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Width' }), {
      target: { value: '50' },
    })
    expect(screen.getByRole('spinbutton', { name: 'Width' })).toHaveValue(50)

    // Load second file.
    vi.mocked(dialogApi.open).mockResolvedValue('/second.nc')
    vi.mocked(gcodeApi.loadGcodeForViewer).mockResolvedValue(secondResult)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open file/i }))
    })

    await waitFor(() => {
      // Override is gone; new metadata value (200) shows.
      expect(screen.getByRole('spinbutton', { name: 'Width' })).toHaveValue(200)
    })
  })
})

// ── Tests: Simulate button enable/disable ─────────────────────────────────────

describe('ToolpathViewerMode — Simulate button enable/disable', () => {
  it('becomes enabled after a file with complete metadata is loaded', async () => {
    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /simulate/i })).toBeEnabled()
    })
  })

  it('remains disabled when required stock fields are missing', async () => {
    const noStockResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      stock: null,
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/partial.nc', noStockResult)

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /simulate/i })).toBeDisabled()
    })
  })

  it('remains disabled when tool type is missing', async () => {
    const noToolResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      tools: [],
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/notool.nc', noToolResult)

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /simulate/i })).toBeDisabled()
    })
  })
})

// ── Tests: field overrides ────────────────────────────────────────────────────

describe('ToolpathViewerMode — field overrides', () => {
  it('sends user-edited diameter to simulateGcodeViewer', async () => {
    vi.mocked(gcodeApi.simulateGcodeViewer).mockResolvedValue(MOCK_MESH)

    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    // Override diameter.
    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: 'Diameter' })).toHaveValue(10)
    })
    fireEvent.change(screen.getByRole('spinbutton', { name: 'Diameter' }), {
      target: { value: '15' },
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /simulate/i }))
    })

    await waitFor(() => {
      expect(gcodeApi.simulateGcodeViewer).toHaveBeenCalledWith(
        '/test/file.nc',
        expect.objectContaining({ width: 100, depth: 100, height: 20 }),
        15,
        'flat_endmill',
        0.5,
      )
    })
  })

  it('resolution slider defaults to 0.5 mm and passes it to simulateGcodeViewer', async () => {
    vi.mocked(gcodeApi.simulateGcodeViewer).mockResolvedValue(MOCK_MESH)

    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    // Do NOT adjust the slider — click simulate directly.
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /simulate/i }))
    })

    await waitFor(() => {
      expect(gcodeApi.simulateGcodeViewer).toHaveBeenCalledWith(
        expect.any(String),
        expect.any(Object),
        expect.any(Number),
        expect.any(String),
        0.5,
      )
    })
  })

  it('clearing an overridden field reverts to the metadata value', async () => {
    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    const diameterInput = screen.getByRole('spinbutton', { name: 'Diameter' })

    // Override to 15.
    fireEvent.change(diameterInput, { target: { value: '15' } })
    expect(diameterInput).toHaveValue(15)

    // Clear to empty — should revert to metadata value 10.
    fireEvent.change(diameterInput, { target: { value: '' } })

    await waitFor(() => {
      expect(diameterInput).toHaveValue(10)
    })
  })
})

// ── Tests: simulate result ────────────────────────────────────────────────────

describe('ToolpathViewerMode — simulate result', () => {
  it('sets simulation mesh on the viewport store on success, leaving toolpathGeometry intact', async () => {
    vi.mocked(gcodeApi.simulateGcodeViewer).mockResolvedValue(MOCK_MESH)

    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    // toolpathGeometry was set by load.
    expect(useViewportStore.getState().toolpathGeometry).toEqual(MOCK_LINE_GEOMETRY)

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /simulate/i }))
    })

    await waitFor(() => {
      expect(useViewportStore.getState().simulationMeshData).toEqual(MOCK_MESH)
      // toolpathGeometry unchanged.
      expect(useViewportStore.getState().toolpathGeometry).toEqual(MOCK_LINE_GEOMETRY)
    })
  })
})

// ── Tests: unsupported tool type ──────────────────────────────────────────────

describe('ToolpathViewerMode — unsupported tool type in metadata', () => {
  it('leaves tool type field empty when metadata type is not in the dropdown', async () => {
    const unsupportedToolResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      tools: [{ number: 1, toolType: 'ball_nose', diameter: 10 }],
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/unsupported.nc', unsupportedToolResult)

    await waitFor(() => {
      expect(screen.getByRole('combobox', { name: 'Tool Type' })).toHaveValue('')
      // Simulate should still be disabled (no tool type selected).
      expect(screen.getByRole('button', { name: /simulate/i })).toBeDisabled()
    })
  })
})

// ── Tests: warnings display ───────────────────────────────────────────────────

describe('ToolpathViewerMode — warnings', () => {
  it('displays warnings from a successful load below the file panel', async () => {
    const warningResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      warnings: [{ line: 5, message: 'Unknown key "foo" ignored' }],
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/warn.nc', warningResult)

    await waitFor(() => {
      expect(screen.getByText(/unknown key "foo" ignored/i)).toBeInTheDocument()
    })
  })

  it('clears warnings when a new file is loaded', async () => {
    const warningResult: GcodeViewerLoadResult = {
      ...MOCK_LOAD_RESULT,
      warnings: [{ line: 5, message: 'Unknown key "foo" ignored' }],
    }

    render(<ToolpathViewerMode />)
    await loadFileViaButton('/warn.nc', warningResult)

    await waitFor(() => {
      expect(screen.getByText(/unknown key "foo" ignored/i)).toBeInTheDocument()
    })

    // Load a second file with no warnings.
    vi.mocked(dialogApi.open).mockResolvedValue('/clean.nc')
    vi.mocked(gcodeApi.loadGcodeForViewer).mockResolvedValue(MOCK_LOAD_RESULT)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open file/i }))
    })

    await waitFor(() => {
      expect(screen.queryByText(/unknown key "foo" ignored/i)).not.toBeInTheDocument()
    })
  })
})

// ── Tests: error states ───────────────────────────────────────────────────────

describe('ToolpathViewerMode — load error', () => {
  it('shows an inline error message when loading fails', async () => {
    vi.mocked(dialogApi.open).mockResolvedValue('/bad.nc')
    vi.mocked(gcodeApi.loadGcodeForViewer).mockRejectedValue({
      kind: 'FileNotFound',
      message: 'File not found',
    })

    render(<ToolpathViewerMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open file/i }))
    })

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('File not found')
    })
  })
})

describe('ToolpathViewerMode — simulate error', () => {
  it('shows an inline error message when simulation fails', async () => {
    vi.mocked(gcodeApi.simulateGcodeViewer).mockRejectedValue({
      kind: 'InvalidInput',
      message: 'Resolution out of range',
    })

    render(<ToolpathViewerMode />)
    await loadFileViaButton()

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /simulate/i }))
    })

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent('Resolution out of range')
    })
  })
})

// ── Tests: back button ────────────────────────────────────────────────────────

describe('ToolpathViewerMode — back button', () => {
  it('calls returnToSelector after checkUnsavedChanges confirms safe', async () => {
    vi.mocked(unsavedGuard.checkUnsavedChanges).mockResolvedValue(true)
    // Spy on the store action before rendering so we get the right object reference.
    const spy = vi.spyOn(useProjectStore.getState(), 'returnToSelector')

    render(<ToolpathViewerMode />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /back/i }))
    })

    await waitFor(() => {
      expect(spy).toHaveBeenCalled()
    })
  })
})
