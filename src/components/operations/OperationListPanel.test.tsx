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
import type { Operation, ProjectSnapshot } from '../../api/types'

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

const opsApi = await import('../../api/operations')
const fileApi = await import('../../api/file')

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

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null })
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
    const drillOp: Operation = { id: 'new-id', name: 'New drill', enabled: true, toolId: TOOL_ID, type: 'drill', params: { depth: 10.0 } }
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
