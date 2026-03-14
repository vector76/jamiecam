/**
 * Tests for OperationListPanel.tsx — operation list, enable/disable toggle,
 * delete, and add operation buttons.
 *
 * The operations and file API modules are mocked so tests run in jsdom
 * without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { OperationListPanel } from './OperationListPanel'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import type { Operation, ProjectSnapshot, LineGeometryData, ToolpathStats, ToolpathProgressEvent } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/operations', () => ({
  editOperation: vi.fn(),
  deleteOperation: vi.fn(),
  addOperation: vi.fn(),
  listOperations: vi.fn(),
  reorderOperations: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

vi.mock('../../api/toolpath', () => ({
  calculateToolpath: vi.fn(),
  getToolpathGeometry: vi.fn(),
  listenToolpathProgress: vi.fn(),
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
  vi.mocked(toolpathApi.listenToolpathProgress).mockResolvedValue(() => {})
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
    expect(screen.getByRole('button', { name: /\+ z-level roughing/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /\+ adaptive clearing/i })).toBeInTheDocument()
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
    expect(screen.getByRole('button', { name: /\+ z-level roughing/i })).toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ adaptive clearing/i })).toBeDisabled()
  })

  it('enables add buttons when tools exist', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: /\+ profile/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ pocket/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ drill/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ z-level roughing/i })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: /\+ adaptive clearing/i })).not.toBeDisabled()
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

  it('add z-level roughing button is disabled when no tools exist', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_NO_TOOLS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: /\+ z-level roughing/i })).toBeDisabled()
  })

  it('add z-level roughing calls addOperation with correct defaults', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    const zlrOp: Operation = { id: 'new-id', name: 'Z-Level Roughing', enabled: true, toolId: TOOL_ID, type: 'z_level_roughing', params: { depth: 5.0, stepdown: 1.0, stepover: 0.5 } }
    vi.mocked(opsApi.addOperation).mockResolvedValue(zlrOp)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ z-level roughing/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'z_level_roughing',
        toolId: TOOL_ID,
        params: expect.objectContaining({ depth: 5.0, stepdown: 1.0, stepover: 0.5 }),
      })
    ))
  })

  it('add adaptive clearing calls addOperation with correct default params', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    const acOp: Operation = { id: 'new-id', name: 'Adaptive Clearing', enabled: true, toolId: TOOL_ID, type: 'adaptive_clearing', params: { depth: 5.0, stepdown: 1.0, optimalLoad: 0.25, stepoverPercent: 50 } }
    vi.mocked(opsApi.addOperation).mockResolvedValue(acOp)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ adaptive clearing/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'adaptive_clearing',
        toolId: TOOL_ID,
        params: expect.objectContaining({ depth: 5.0, stepdown: 1.0, optimalLoad: 0.25, stepoverPercent: 50 }),
      })
    ))
  })

  it('add parallel finishing calls addOperation with correct default params', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    const pfOp: Operation = { id: 'new-id', name: 'Parallel Finishing', enabled: true, toolId: TOOL_ID, type: 'parallelFinishing', params: { stepover: 0.5, directionAngleDeg: 0, allowance: 0 } }
    vi.mocked(opsApi.addOperation).mockResolvedValue(pfOp)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: /\+ parallel finishing/i }))

    await waitFor(() => expect(opsApi.addOperation).toHaveBeenCalledWith(
      expect.objectContaining({
        type: 'parallelFinishing',
        toolId: TOOL_ID,
        params: expect.objectContaining({ stepover: 0.5, directionAngleDeg: 0, allowance: 0 }),
      })
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

// ── Stale indicator ───────────────────────────────────────────────────────────

describe('OperationListPanel — stale indicator', () => {
  it('renders stale indicator when needsRecalculate is true', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getAllByLabelText('stale')).toHaveLength(2)
  })

  it('does not render stale indicator when needsRecalculate is false', () => {
    const SNAPSHOT_NO_STALE: ProjectSnapshot = {
      ...SNAPSHOT_WITH_OPS,
      operations: [
        { ...SNAPSHOT_WITH_OPS.operations[0], needsRecalculate: false },
        { ...SNAPSHOT_WITH_OPS.operations[1], needsRecalculate: false },
      ],
    }
    useProjectStore.setState({ snapshot: SNAPSHOT_NO_STALE })
    render(<OperationListPanel />)
    expect(screen.queryByLabelText('stale')).not.toBeInTheDocument()
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

  it('Calculate is disabled for drill operations even when stock is defined', async () => {
    const DRILL_OP_ID = 'dddd-0001'
    const drillOp = { id: DRILL_OP_ID, name: 'Drill Op', operationType: 'drill' as const, enabled: true, needsRecalculate: false }
    const fullDrillOp: Operation = { id: DRILL_OP_ID, name: 'Drill Op', enabled: true, toolId: TOOL_ID, type: 'drill', params: { depth: 10.0, points: [] } }
    vi.mocked(opsApi.listOperations).mockResolvedValue([fullDrillOp])
    useProjectStore.setState({
      snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [drillOp] }
    })
    render(<OperationListPanel />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Drill Op' })).toBeDisabled())
  })

  it('Calculate is disabled for a drill operation with 0 points (even if stock is set)', async () => {
    const DRILL_OP_ID = 'dddd-0001'
    const drillOp = { id: DRILL_OP_ID, name: 'Drill Op', operationType: 'drill' as const, enabled: true, needsRecalculate: false }
    const fullDrillOp: Operation = { id: DRILL_OP_ID, name: 'Drill Op', enabled: true, toolId: TOOL_ID, type: 'drill', params: { depth: 10.0, points: [] } }
    vi.mocked(opsApi.listOperations).mockResolvedValue([fullDrillOp])
    useProjectStore.setState({ snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [drillOp] } })
    render(<OperationListPanel />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Drill Op' })).toBeDisabled())
  })

  it('Calculate is enabled for a drill operation when stock is set and points list is non-empty', async () => {
    const DRILL_OP_ID = 'dddd-0001'
    const drillOp = { id: DRILL_OP_ID, name: 'Drill Op', operationType: 'drill' as const, enabled: true, needsRecalculate: false }
    const fullDrillOp: Operation = { id: DRILL_OP_ID, name: 'Drill Op', enabled: true, toolId: TOOL_ID, type: 'drill', params: { depth: 10.0, points: [{ x: 10, y: 20 }] } }
    vi.mocked(opsApi.listOperations).mockResolvedValue([fullDrillOp])
    useProjectStore.setState({ snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [drillOp] } })
    render(<OperationListPanel />)
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Drill Op' })).not.toBeDisabled())
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

  it('Calculate refreshes project snapshot after success', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue({ ...SNAPSHOT_WITH_STOCK, projectName: 'Updated' })

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))

    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('Updated')
  })

  it('Calculate is enabled for adaptive clearing operations when stock is defined', () => {
    const AC_OP_ID = 'cccc-0001'
    const acOp = { id: AC_OP_ID, name: 'Adaptive Clearing', operationType: 'adaptive_clearing' as const, enabled: true, needsRecalculate: true }
    useProjectStore.setState({ snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [acOp] } })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Adaptive Clearing' })).not.toBeDisabled()
  })

  it('Calculate is enabled for parallel finishing operations when stock is defined', () => {
    const PF_OP_ID = 'dddd-0001'
    const pfOp = { id: PF_OP_ID, name: 'Parallel Finishing', operationType: 'parallelFinishing' as const, enabled: true, needsRecalculate: true }
    useProjectStore.setState({ snapshot: { ...SNAPSHOT_WITH_STOCK, operations: [pfOp] } })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Calculate Parallel Finishing' })).not.toBeDisabled()
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

// ── Calculate loading state ───────────────────────────────────────────────────

describe('OperationListPanel — calculate loading state', () => {
  it('shows loading label on active row while calculation is in flight', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    let resolveCalc: () => void
    const deferred = new Promise<ToolpathStats>((res) => { resolveCalc = () => res(TOOLPATH_STATS) })
    vi.mocked(toolpathApi.calculateToolpath).mockReturnValue(deferred)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))
    // before resolving: button text changes to loading indicator and is disabled
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).toHaveTextContent('…'))
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).toBeDisabled()
    // resolve promise and verify label reverts
    resolveCalc!()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).toHaveTextContent('Calc'))
  })

  it('all other Calculate buttons are disabled while any calculation is in flight', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    const deferred = new Promise<ToolpathStats>(() => { /* never resolves */ })
    vi.mocked(toolpathApi.calculateToolpath).mockReturnValue(deferred)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))
    // OP1's button should also be disabled while OP2 is calculating
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Outer Profile' })).toBeDisabled())
  })

  it('Calculate button re-enabled after successful calculation', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    let resolveCalc: () => void
    const deferred = new Promise<ToolpathStats>((res) => { resolveCalc = () => res(TOOLPATH_STATS) })
    vi.mocked(toolpathApi.calculateToolpath).mockReturnValue(deferred)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))
    resolveCalc!()
    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate Outer Profile' })).not.toBeDisabled())
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).not.toBeDisabled()
  })

  it('Calculate button re-enabled after calculation error', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.calculateToolpath).mockRejectedValue({ kind: 'CalcFailed', message: 'calc error' })
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Rough Pocket' }))
    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('calc error'))
    expect(screen.getByRole('button', { name: 'Calculate Rough Pocket' })).not.toBeDisabled()
  })
})

// ── Reorder ───────────────────────────────────────────────────────────────────

describe('OperationListPanel — reorder', () => {
  it('up button is disabled for first operation', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Move up Outer Profile' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Move down Outer Profile' })).not.toBeDisabled()
  })

  it('down button is disabled for last operation', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    render(<OperationListPanel />)
    expect(screen.getByRole('button', { name: 'Move down Rough Pocket' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Move up Rough Pocket' })).not.toBeDisabled()
  })

  it('up click on second operation calls reorderOperations with swapped order', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.reorderOperations).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Move up Rough Pocket' }))
    await waitFor(() => expect(opsApi.reorderOperations).toHaveBeenCalledWith([OP2_ID, OP1_ID]))
  })

  it('down click on first operation calls reorderOperations with swapped order', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.reorderOperations).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Move down Outer Profile' }))
    await waitFor(() => expect(opsApi.reorderOperations).toHaveBeenCalledWith([OP2_ID, OP1_ID]))
  })

  it('snapshot is refreshed after successful reorder', async () => {
    const updated = { ...SNAPSHOT_WITH_OPS, projectName: 'After Reorder' }
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.reorderOperations).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(updated)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Move down Outer Profile' }))
    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('After Reorder')
  })

  it('error notification pushed when reorderOperations rejects', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS })
    vi.mocked(opsApi.reorderOperations).mockRejectedValue({ kind: 'SaveFailed', message: 'reorder error' })
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Move down Outer Profile' }))
    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('reorder error'))
  })

  it('reorder button click does not trigger row selection', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_OPS, selectedOperationId: null })
    vi.mocked(opsApi.reorderOperations).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_OPS)
    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Move down Outer Profile' }))
    await waitFor(() => expect(opsApi.reorderOperations).toHaveBeenCalled())
    expect(useProjectStore.getState().selectedOperationId).toBeNull()
  })
})

// ── Progress bar ──────────────────────────────────────────────────────────────

describe('OperationListPanel — progress bar', () => {
  it('progress element appears when operation is calculating and updates on event', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    let progressHandler: ((event: ToolpathProgressEvent) => void) | null = null
    vi.mocked(toolpathApi.listenToolpathProgress).mockImplementation(async (handler) => {
      progressHandler = handler
      return () => {}
    })
    const deferred = new Promise<ToolpathStats>(() => { /* never resolves */ })
    vi.mocked(toolpathApi.calculateToolpath).mockReturnValue(deferred)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Outer Profile' }))

    await waitFor(() => expect(progressHandler).not.toBeNull())
    act(() => { progressHandler!({ operationId: OP1_ID, percent: 50, message: '' }) })

    const el = screen.getByRole('progressbar', { name: `Progress for Outer Profile` })
    expect(el).toBeInTheDocument()
    expect(el).toHaveAttribute('value', '50')
  })

  it('progress element is not rendered after calculation completes', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK })
    vi.mocked(toolpathApi.listenToolpathProgress).mockResolvedValue(() => {})
    vi.mocked(toolpathApi.calculateToolpath).mockResolvedValue(TOOLPATH_STATS)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)

    render(<OperationListPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Calculate Outer Profile' }))

    await waitFor(() => expect(toolpathApi.calculateToolpath).toHaveBeenCalled())
    await waitFor(() => expect(screen.queryByRole('progressbar')).not.toBeInTheDocument())
  })
})
