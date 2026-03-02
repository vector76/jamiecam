/**
 * Tests for OperationListPanel.tsx — operation list, enable/disable toggle,
 * delete, and add operation buttons.
 *
 * The operations and file API modules are mocked so tests run in jsdom
 * without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { OperationListPanel } from './OperationListPanel'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import type { Operation, ProjectSnapshot, LineGeometryData, ToolpathStats } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/operations', () => ({
  editOperation: vi.fn(),
  deleteOperation: vi.fn(),
  addOperation: vi.fn(),
  listOperations: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

vi.mock('../../api/toolpath', () => ({
  calculateToolpath: vi.fn(),
  getToolpathGeometry: vi.fn(),
}))

vi.mock('./OperationEditorForm', () => ({
  OperationEditorForm: ({ operationId }: { operationId: string | null }) =>
    <div data-testid="editor-form" data-op-id={operationId ?? ''} />,
}))

const opsApi = await import('../../api/operations')
const fileApi = await import('../../api/file')
const toolpathApi = await import('../../api/toolpath')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const TOOL_ID = 'aaaaaaaa-0000-0000-0000-000000000001'
const OP1_ID = 'bbbbbbbb-0000-0000-0000-000000000001'
const OP2_ID = 'bbbbbbbb-0000-0000-0000-000000000002'

const SNAPSHOT_WITH_OPS: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test',
  modifiedAt: '',
  tools: [{ id: TOOL_ID, name: '10mm Flat Endmill', toolType: 'flat_endmill' }],
  stock: null,
  wcs: [],
  operations: [
    { id: OP1_ID, name: 'Outer Profile', operationType: 'profile', enabled: true, needsRecalculate: true },
    { id: OP2_ID, name: 'Rough Pocket', operationType: 'pocket', enabled: false, needsRecalculate: true },
  ],
}

const SNAPSHOT_NO_TOOLS: ProjectSnapshot = {
  ...SNAPSHOT_WITH_OPS,
  tools: [],
}

const FULL_OP1: Operation = {
  id: OP1_ID,
  name: 'Outer Profile',
  enabled: true,
  toolId: TOOL_ID,
  type: 'profile',
  params: { depth: 10.0, stepdown: 2.5, compensationSide: 'left' },
}

const FULL_OP2: Operation = {
  id: OP2_ID,
  name: 'Rough Pocket',
  enabled: false,
  toolId: TOOL_ID,
  type: 'pocket',
  params: { depth: 15.0, stepdown: 3.0, stepoverPercent: 45.0 },
}

// ── Setup ─────────────────────────────────────────────────────────────────────

const STOCK = { type: 'box' as const, origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 20, height: 50 }

const SNAPSHOT_WITH_STOCK: ProjectSnapshot = {
  ...SNAPSHOT_WITH_OPS,
  stock: STOCK,
}

const TOOLPATH_STATS: ToolpathStats = { totalPassCount: 3, totalPointCount: 42, totalPathLengthMm: 150.5 }

const LINE_GEOMETRY: LineGeometryData = { positions: [0, 0, 0, 1, 1, 1], colours: [1, 0, 0, 0, 1, 0], types: [0] }

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
  useViewportStore.setState({ toolpathGeometry: null })
})

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('OperationListPanel — rendering', () => {
  it('renders all operations from the store in order', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByText('Outer Profile')).toBeInTheDocument()
    expect(screen.getByText('Rough Pocket')).toBeInTheDocument()
  })

  it('renders operation type labels', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByText('profile')).toBeInTheDocument()
    expect(screen.getByText('pocket')).toBeInTheDocument()
  })

  it('renders enabled state on checkboxes', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    const toggle1 = screen.getByRole('checkbox', { name: 'Toggle Outer Profile' })
    const toggle2 = screen.getByRole('checkbox', { name: 'Toggle Rough Pocket' })
    expect(toggle1).toBeChecked()
    expect(toggle2).not.toBeChecked()
  })

  it('renders add buttons for each operation type', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: /\+ profile/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ pocket/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ drill/i })).toBeInTheDocument()
  })

  it('renders nothing when operation list is empty', () => {
    useProjectStore.setState({ snapshot: { ...SNAPSHOT_WITH_OPS, operations: [] } })
    render(<OperationListPanel />)
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument()
  })
})

// ── Add buttons disabled/enabled ──────────────────────────────────────────────

describe('OperationListPanel — add buttons', () => {
  it('disables add buttons when no tools exist', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_NO_TOOLS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: /\+ profile/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ pocket/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ drill/i })).toBeDisabled()
  })

  it('enables add buttons when tools exist', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: /\+ profile/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ pocket/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ drill/i })).not.toBeDisabled()
  })

  it('add profile calls addOperation with type profile and first tool ID', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.addOperation).mockResolvedValue({ ...FULL_OP1, id: 'new-id' })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ profile/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'profile', toolId: TOOL_ID })
    ))
  })

  it('add pocket calls addOperation with type pocket', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.addOperation).mockResolvedValue({ ...FULL_OP2, id: 'new-id' })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ pocket/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'pocket', toolId: TOOL_ID })
    ))
  })

  it('add drill calls addOperation with type drill', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    const drillOp: Operation = { id: 'new-id', name: 'New drill', enabled: true, toolId: TOOL_ID, type: 'drill', params: { depth: 10.0, points: [] } }
    vi.mocked(opsApi.addOperation).mockResolvedValue(drillOp)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ drill/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({ type: 'drill', toolId: TOOL_ID })
    ))
  })

  it('add button refreshes snapshot after addOperation', async () => {
    const newSnapshot = { ...SNAPSHOT_WITH_OPS, projectName: 'Updated' }
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.addOperation).mockResolvedValue({ ...FULL_OP1, id: 'new-id' })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(newSnapshot)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ profile/i }))

    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('Updated')
  })
})

// ── Toggle enabled ────────────────────────────────────────────────────────────

describe('OperationListPanel — enable/disable toggle', () => {
  it('toggle calls listOperations then editOperation with flipped enabled', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.listOperations).mockResolvedValue([FULL_OP1, FULL_OP2])
    vi.mocked(opsApi.editOperation).mockResolvedValue({ ...FULL_OP1, enabled: false })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('checkbox', { name: 'Toggle Outer Profile' }))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      OP1_ID,
      expect.objectContaining({ enabled: false })
    ))
  })

  it('toggle refreshes snapshot after editOperation', async () => {
    const newSnapshot = { ...SNAPSHOT_WITH_OPS, projectName: 'After Toggle' }
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.listOperations).mockResolvedValue([FULL_OP1, FULL_OP2])
    vi.mocked(opsApi.editOperation).mockResolvedValue({ ...FULL_OP1, enabled: false })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(newSnapshot)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('checkbox', { name: 'Toggle Outer Profile' }))

    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('After Toggle')
  })
})

// ── Delete ────────────────────────────────────────────────────────────────────

describe('OperationListPanel — delete', () => {
  it('delete button calls deleteOperation with correct ID', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.deleteOperation).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Delete Outer Profile' }))

    await waitFor(() => expect(opsApi.deleteOperation).toHaveBeenCalledWith(OP1_ID))
  })

  it('delete button refreshes snapshot after deleteOperation', async () => {
    const newSnapshot = { ...SNAPSHOT_WITH_OPS, projectName: 'After Delete' }
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.deleteOperation).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(newSnapshot)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Delete Rough Pocket' }))

    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('After Delete')
  })
})

// ── Selection ─────────────────────────────────────────────────────────────────

describe('OperationListPanel — selection', () => {
  it('clicking a row sets selectedOperationId in the store', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    fireEvent.click(screen.getByText('Rough Pocket'))
    expect(useProjectStore.getState().selectedOperationId).toBe(OP2_ID)
  })

  it('selected row has a different background', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS, selectedOperationId: OP1_ID })
    render(<OperationListPanel />)
    const row = screen.getByText('Outer Profile').closest('div')!
    expect(row.style.background).toBeTruthy()
  })

  it('mounts OperationEditorForm with selectedOpId', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS, selectedOperationId: OP2_ID })
    render(<OperationListPanel />)
    expect(screen.getByTestId('editor-form')).toHaveAttribute('data-op-id', OP2_ID)
  })
})

// ── Calculate button ──────────────────────────────────────────────────────────

describe('OperationListPanel — Calculate button', () => {
  it('renders a Calculate button for each operation', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Outer Profile' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).toBeInTheDocument()
  })

  it('Calculate is enabled for profile operations when stock is defined', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Outer Profile' })).not.toBeDisabled()
  })

  it('Calculate is disabled for drill operations even when stock is defined', () => {
    const drillOp = { id: 'dddd-0001', name: 'Drill Op', operationType: 'drill' as const, enabled: true, needsRecalculate: false }
    useProjectStore.setState({
      snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [drillOp] }
    })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Drill Op' })).toBeDisabled()
  })

  it('Calculate is disabled when stock is null even for pocket operations', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS }) // stock: null
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).toBeDisabled()
  })

  it('Calculate is enabled for pocket operations when stock is set', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).not.toBeDisabled()
  })

  it('Calculate calls calculateToolpath and getToolpathGeometry', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))

    await waitFor(() => expect(toolpathApi.calculateToolpath).toHaveBeenCalledWith(OP2_ID))
    expect(toolpathApi.getToolpathGeometry).toHaveBeenCalledWith(OP2_ID)
  })

  it('Calculate sets toolpath geometry in viewport store', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))

    await waitFor(() => expect(useViewportStore.getState().toolpathGeometry).toEqual(LINE_GEOMETRY))
  })

  it('Calculate pushes a stats notification', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))

    await waitFor(() => {
      const notes = useProjectStore.getState().notifications
      expect(notes.some((n) => n.includes('3 passes') && n.includes('42 pts') && n.includes('150.5 mm'))).toBe(true)
    })
  })

  it('Calculate button click does not also select the row via row click', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK, selectedOperationId: null })
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))

    await waitFor(() => expect(toolpathApi.calculateToolpath).toHaveBeenCalled())
    // row-level onClick should NOT have fired due to stopPropagation
    expect(useProjectStore.getState().selectedOperationId).toBeNull()
  })
})
