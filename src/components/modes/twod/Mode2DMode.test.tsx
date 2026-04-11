/**
 * Tests for Mode2DMode.tsx — 2D Profiling mode top-level component.
 *
 * All Tauri IPC calls and child components that require browser APIs are
 * mocked. The real Zustand projectStore is used so snapshot-driven state
 * (stock, safeHeight, artworkOrigin) is verifiable.
 *
 * Canvas2D is mocked to capture the onCurveSelect callback so tests can
 * simulate curve selection without a real canvas.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { Mode2DMode } from './Mode2DMode'
import { useProjectStore } from '../../../store/projectStore'
import type { ProjectSnapshot } from '../../../api/types'

// ── Canvas2D mock — captures onCurveSelect for use in tests ──────────────────

let capturedCurveSelect: ((id: string | null) => void) | null = null

vi.mock('./Canvas2D', () => ({
  Canvas2D: (props: { onCurveSelect?: (id: string | null) => void }) => {
    capturedCurveSelect = props.onCurveSelect ?? null
    return <div data-testid="canvas-2d">Canvas2D</div>
  },
}))

vi.mock('../../../viewport/Viewport', () => ({
  Viewport: (props: { className?: string }) => (
    <div data-testid="viewport" className={props.className}>Viewport</div>
  ),
}))

// ── Other module mocks ────────────────────────────────────────────────────────

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}))

vi.mock('../../../api/twodMode', () => ({
  loadTwodFile: vi.fn(),
  getTwodCurves: vi.fn(),
  setSafeHeight: vi.fn(),
  setArtworkOrigin: vi.fn(),
  generate2dGcode: vi.fn(),
}))

vi.mock('../../../api/stock', () => ({
  setStock: vi.fn(),
  getStock: vi.fn(),
}))

vi.mock('../../../api/operations', () => ({
  addOperation: vi.fn(),
  editOperation: vi.fn(),
  deleteOperation: vi.fn(),
  listOperations: vi.fn(),
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

const MOCK_TOOL = {
  id: 'tool-1',
  name: '6mm Flat',
  type: 'flat_endmill',
  diameter: 6,
  cuttingLength: 20,
  shankDiameter: 6,
}

const MOCK_OPERATION = {
  id: 'op-1',
  name: 'Profile 2D',
  enabled: true,
  toolId: 'tool-1',
  type: 'profile_2d' as const,
  params: {
    curveId: 'curve-1',
    cutType: 'outside' as const,
    direction: 'climb' as const,
    topOfCut: 5.0,
    depthOfCut: 3.0,
    stepDown: 1.0,
    feedRate: 1000.0,
  },
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  capturedCurveSelect = null
  vi.mocked(twodApi.getTwodCurves).mockResolvedValue(null)
  vi.mocked(twodApi.loadTwodFile).mockResolvedValue(MOCK_LOAD_RESULT)
  vi.mocked(twodApi.setSafeHeight).mockResolvedValue(undefined)
  vi.mocked(twodApi.setArtworkOrigin).mockResolvedValue(undefined)
  vi.mocked(twodApi.generate2dGcode).mockResolvedValue({
    gcode: 'G0 Z5',
    lineGeometry: { positions: [], colours: [], types: [] },
    warnings: [],
    stats: { totalPointCount: 0, totalPassCount: 0, totalPathLengthMm: 0 },
  })
  vi.mocked(stockApi.setStock).mockResolvedValue(undefined)
  vi.mocked(operationsApi.addOperation).mockResolvedValue(MOCK_OPERATION)
  vi.mocked(operationsApi.editOperation).mockResolvedValue(MOCK_OPERATION)
  vi.mocked(operationsApi.deleteOperation).mockResolvedValue(undefined)
  vi.mocked(operationsApi.listOperations).mockResolvedValue([])
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

// ── Helper: select a curve via the captured Canvas2D callback ─────────────────

async function selectCurve(id: string) {
  await act(async () => {
    capturedCurveSelect?.(id)
  })
}

// ── Tests: initial render ─────────────────────────────────────────────────────

describe('Mode2DMode — initial render', () => {
  it('renders the Load 2D File button when no file is loaded', async () => {
    render(<Mode2DMode />)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /load 2d file/i })).toBeInTheDocument()
    })
  })

  it('does not show a file info line before a file is loaded', async () => {
    render(<Mode2DMode />)
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

    expect(vi.mocked(twodApi.loadTwodFile).mock.calls.length).toBe(callCount)
  })
})

// ── Tests: operations preserved on failed load ────────────────────────────────

describe('Mode2DMode — operations preserved when new file load fails', () => {
  it('does not call deleteOperation when loadTwodFile rejects', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton('/first.dxf')

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

    vi.mocked(twodApi.loadTwodFile).mockRejectedValue({
      kind: 'FileNotFound',
      message: 'File not found',
    })
    vi.mocked(dialogApi.open).mockResolvedValue('/second.dxf')

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

    expect(stockApi.setStock).not.toHaveBeenCalled()
  })
})

// ── Tests: operations panel ───────────────────────────────────────────────────

describe('Mode2DMode — operations panel', () => {
  it('shows the "click a closed curve" message when no curve is selected', async () => {
    render(<Mode2DMode />)
    await waitFor(() => {
      expect(
        screen.getByText(/click a closed curve on the canvas to assign a cut operation/i),
      ).toBeInTheDocument()
    })
  })

  it('"Add operation" button is disabled when tool library is empty', async () => {
    vi.mocked(toolsApi.listTools).mockResolvedValue([])
    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /add operation/i })).toBeDisabled()
    })
    expect(screen.getByText(/add a tool to the project first/i)).toBeInTheDocument()
  })

  it('clicking "Add operation" calls addOperation with selected curve ID', async () => {
    vi.mocked(toolsApi.listTools).mockResolvedValue([MOCK_TOOL])
    vi.mocked(operationsApi.addOperation).mockResolvedValue(MOCK_OPERATION)

    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /add operation/i })).not.toBeDisabled()
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /add operation/i }))
    })

    await waitFor(() => {
      expect(operationsApi.addOperation).toHaveBeenCalledWith(
        expect.objectContaining({
          type: 'profile_2d',
          params: expect.objectContaining({ curveId: 'curve-1' }),
          toolId: 'tool-1',
        }),
      )
    })
  })

  it('edit form renders with correct values for an existing operation', async () => {
    vi.mocked(toolsApi.listTools).mockResolvedValue([MOCK_TOOL])
    vi.mocked(operationsApi.listOperations).mockResolvedValue([MOCK_OPERATION])
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    // Edit form should load the operation and show the feed rate field
    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: /feed rate/i })).toBeInTheDocument()
    })
    expect(screen.getByRole('spinbutton', { name: /feed rate/i })).toHaveValue(1000)
  })

  it('feed rate field change calls editOperation on blur', async () => {
    vi.mocked(toolsApi.listTools).mockResolvedValue([MOCK_TOOL])
    vi.mocked(operationsApi.listOperations).mockResolvedValue([MOCK_OPERATION])
    vi.mocked(operationsApi.editOperation).mockResolvedValue({
      ...MOCK_OPERATION,
      params: { ...MOCK_OPERATION.params, feedRate: 1500 },
    })
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: /feed rate/i })).toBeInTheDocument()
    })

    fireEvent.change(screen.getByRole('spinbutton', { name: /feed rate/i }), {
      target: { value: '1500' },
    })
    fireEvent.blur(screen.getByRole('spinbutton', { name: /feed rate/i }))

    await waitFor(() => {
      expect(operationsApi.editOperation).toHaveBeenCalledWith(
        'op-1',
        expect.objectContaining({
          params: expect.objectContaining({ feedRate: 1500 }),
        }),
      )
    })
  })

  it('"Remove operation" calls deleteOperation and clears the form', async () => {
    vi.mocked(toolsApi.listTools).mockResolvedValue([MOCK_TOOL])
    vi.mocked(operationsApi.listOperations).mockResolvedValue([MOCK_OPERATION])
    vi.mocked(operationsApi.deleteOperation).mockResolvedValue(undefined)
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /remove operation/i })).toBeInTheDocument()
    })

    // Simulate snapshot update after delete (no more ops for curve-1)
    vi.mocked(operationsApi.deleteOperation).mockImplementationOnce(async () => {
      useProjectStore.setState({
        snapshot: { ...BASE_SNAPSHOT, operations: [] },
      })
    })

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /remove operation/i }))
    })

    await waitFor(() => {
      expect(operationsApi.deleteOperation).toHaveBeenCalledWith('op-1')
    })
  })

  it('multi-tool advisory shown when two ops have different tool IDs', async () => {
    const tool2 = { ...MOCK_TOOL, id: 'tool-2', name: '4mm Ball' }
    const op2 = { ...MOCK_OPERATION, id: 'op-2', toolId: 'tool-2', params: { ...MOCK_OPERATION.params, curveId: 'curve-2' } }
    vi.mocked(toolsApi.listTools).mockResolvedValue([MOCK_TOOL, tool2])
    vi.mocked(operationsApi.listOperations).mockResolvedValue([MOCK_OPERATION, op2])
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          { id: 'op-1', name: 'Profile 2D', operationType: 'profile_2d', enabled: true, needsRecalculate: false, curveId: 'curve-1' },
          { id: 'op-2', name: 'Profile 2D 2', operationType: 'profile_2d', enabled: true, needsRecalculate: false, curveId: 'curve-2' },
        ],
      },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()
    await selectCurve('curve-1')

    await waitFor(() => {
      expect(screen.getByRole('spinbutton', { name: /feed rate/i })).toBeInTheDocument()
    })

    await waitFor(() => {
      expect(
        screen.getByText(/multiple tools assigned/i),
      ).toBeInTheDocument()
    })
  })
})

// ── Tests: generate button ────────────────────────────────────────────────────

describe('Mode2DMode — generate G-code button', () => {
  it('generate button is disabled when no file is loaded', async () => {
    render(<Mode2DMode />)
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /generate g-code/i })).toBeDisabled()
    })
  })

  it('generate button is disabled when no enabled Profile2d operations exist', async () => {
    render(<Mode2DMode />)
    await loadDxfViaButton()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /generate g-code/i })).toBeDisabled()
    })
  })

  it('generate button is enabled when at least one enabled Profile2d op exists', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()

    await waitFor(() => {
      expect(screen.getByRole('button', { name: /generate g-code/i })).not.toBeDisabled()
    })
  })

  it('on generate success: transitions to viewing subState', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    vi.mocked(twodApi.generate2dGcode).mockResolvedValue({
      gcode: 'G0 Z5\nG1 X10',
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings: [],
      stats: { totalPointCount: 10, totalPassCount: 1, totalPathLengthMm: 100 },
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /generate g-code/i }))
    })

    await waitFor(() => {
      expect(twodApi.generate2dGcode).toHaveBeenCalledWith('grbl')
    })

    // After success, editing substate is hidden — generate button is gone
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /generate g-code/i })).not.toBeInTheDocument()
    })
  })

  it('generate error is cleared when a new file is loaded', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          { id: 'op-1', name: 'Profile 2D', operationType: 'profile_2d', enabled: true, needsRecalculate: false, curveId: 'curve-1' },
        ],
      },
    })
    vi.mocked(twodApi.generate2dGcode).mockRejectedValue({ kind: 'GenerationFailed', message: 'Tool mismatch' })

    render(<Mode2DMode />)
    await loadDxfViaButton()

    // Trigger a generate error
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /generate g-code/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
    })

    // Load a new file — first we get the replace confirm dialog (a file is already loaded),
    // then confirm it. The error should disappear after the new file loads.
    vi.mocked(dialogApi.open).mockResolvedValue('/fresh.dxf')
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
      expect(twodApi.loadTwodFile).toHaveBeenCalledWith('/fresh.dxf', null)
    })

    await waitFor(() => {
      expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    })
  })

  it('on generate error: error message is displayed and state remains editing', async () => {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })

    vi.mocked(twodApi.generate2dGcode).mockRejectedValue({
      kind: 'GenerationFailed',
      message: 'Multiple tools in use',
    })

    render(<Mode2DMode />)
    await loadDxfViaButton()

    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /generate g-code/i }))
    })

    await waitFor(() => {
      expect(screen.getByRole('alert')).toBeInTheDocument()
      expect(screen.getByRole('alert')).toHaveTextContent(/multiple tools in use/i)
    })

    // Still in editing subState — generate button still visible
    expect(screen.getByRole('button', { name: /generate g-code/i })).toBeInTheDocument()
  })
})

// ── Tests: viewing sub-state ──────────────────────────────────────────────────

describe('Mode2DMode — viewing sub-state', () => {
  async function generateAndView(gcode = 'G0 Z5\nG1 X10', warnings: string[] = []) {
    useProjectStore.setState({
      snapshot: {
        ...BASE_SNAPSHOT,
        operations: [
          {
            id: 'op-1',
            name: 'Profile 2D',
            operationType: 'profile_2d',
            enabled: true,
            needsRecalculate: false,
            curveId: 'curve-1',
          },
        ],
      },
    })
    vi.mocked(twodApi.generate2dGcode).mockResolvedValue({
      gcode,
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings,
      stats: { totalPointCount: 10, totalPassCount: 1, totalPathLengthMm: 100 },
    })
    render(<Mode2DMode />)
    await loadDxfViaButton()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /generate g-code/i }))
    })
    await waitFor(() => {
      expect(screen.queryByRole('button', { name: /generate g-code/i })).not.toBeInTheDocument()
    })
  }

  it('canvas is not rendered when subState is viewing', async () => {
    await generateAndView()
    expect(screen.queryByTestId('canvas-2d')).not.toBeInTheDocument()
  })

  it('G-code text is displayed when subState is viewing', async () => {
    await generateAndView('G0 Z5\nG1 X10')
    expect(screen.getByText(/G0 Z5/)).toBeInTheDocument()
  })

  it('warnings banner is shown when warnings are non-empty', async () => {
    await generateAndView('G0 Z5', ['Top of cut below stock'])
    await waitFor(() => {
      expect(screen.getByRole('status')).toBeInTheDocument()
      expect(screen.getByRole('status')).toHaveTextContent(/top of cut below stock/i)
    })
  })

  it('warnings banner is absent when warnings are empty', async () => {
    await generateAndView('G0 Z5', [])
    expect(screen.queryByRole('status')).not.toBeInTheDocument()
  })

  it('"Back to 2D Canvas" click returns to editing state', async () => {
    await generateAndView()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /back to 2d canvas/i }))
    })
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /generate g-code/i })).toBeInTheDocument()
    })
  })

  it('operations panel is still visible after returning to editing', async () => {
    await generateAndView()
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /back to 2d canvas/i }))
    })
    await waitFor(() => {
      expect(
        screen.getByText(/click a closed curve on the canvas to assign a cut operation/i),
      ).toBeInTheDocument()
    })
  })
})
