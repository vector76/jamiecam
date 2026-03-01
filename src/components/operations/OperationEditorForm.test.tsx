/**
 * Tests for OperationEditorForm.tsx — renders forms for editing pocket and
 * profile operations, handles null/unknown states, and calls the API on field change.
 *
 * API modules are mocked so tests run in jsdom without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { OperationEditorForm } from './OperationEditorForm'
import { useProjectStore } from '../../store/projectStore'
import type { Operation, ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/operations', () => ({
  listOperations: vi.fn(),
  editOperation: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

const opsApi = await import('../../api/operations')
const fileApi = await import('../../api/file')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const TOOL_ID_A = 'aaaaaaaa-0000-0000-0000-000000000001'
const TOOL_ID_B = 'aaaaaaaa-0000-0000-0000-000000000002'
const POCKET_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000001'
const PROFILE_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000002'

const POCKET_OP: Operation = {
  id: POCKET_OP_ID,
  name: 'Rough Pocket',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'pocket',
  params: { depth: 5.0, stepdown: 1.0, stepoverPercent: 50.0 },
}

const PROFILE_OP: Operation = {
  id: PROFILE_OP_ID,
  name: 'Outer Profile',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'profile',
  params: { depth: 10.0, stepdown: 2.5, compensationSide: 'left' },
}

const SNAPSHOT_BASE: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test',
  modifiedAt: '',
  tools: [
    { id: TOOL_ID_A, name: '10mm Flat', toolType: 'flat_endmill' },
    { id: TOOL_ID_B, name: '6mm Ball', toolType: 'ball_nose' },
  ],
  stock: null,
  wcs: [],
  operations: [],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: SNAPSHOT_BASE })
})

// ── Null / empty state ─────────────────────────────────────────────────────────

describe('OperationEditorForm — null state', () => {
  it('renders empty state when operationId is null', () => {
    render(<OperationEditorForm operationId={null} />)
    expect(screen.getByText('Select an operation to edit')).toBeInTheDocument()
  })

  it('does not call listOperations when operationId is null', () => {
    render(<OperationEditorForm operationId={null} />)
    expect(opsApi.listOperations).not.toHaveBeenCalled()
  })
})

// ── Profile form rendering ─────────────────────────────────────────────────────

describe('OperationEditorForm — profile form', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([PROFILE_OP])
  })

  it('renders depth, stepdown, and compensation side inputs for a profile operation', async () => {
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    expect(screen.getByLabelText('Step-down (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Compensation side')).toBeInTheDocument()
  })

  it('depth and stepdown inputs have correct default values', async () => {
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toHaveValue(10))
    expect(screen.getByLabelText('Step-down (mm)')).toHaveValue(2.5)
  })

  it('calls editOperation with updated compensationSide on select change', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Compensation side')).toBeInTheDocument())
    fireEvent.change(screen.getByLabelText('Compensation side'), { target: { value: 'right' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ compensationSide: 'right' }) }),
    ))
  })

  it('calls editOperation with updated depth on blur', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Floor depth (mm)'), { target: { value: '15' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ depth: 15 }) }),
    ))
  })

  it('calls editOperation with updated stepdown on blur', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Step-down (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Step-down (mm)'), { target: { value: '1.5' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepdown: 1.5 }) }),
    ))
  })
})

// ── Pocket form rendering ──────────────────────────────────────────────────────

describe('OperationEditorForm — pocket form', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
  })

  it('renders tool select with the tools from the store', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox')).toBeInTheDocument())
    expect(screen.getByRole('option', { name: '10mm Flat' })).toBeInTheDocument()
    expect(screen.getByRole('option', { name: '6mm Ball' })).toBeInTheDocument()
  })

  it('renders depth, stepdown, and stepover inputs with correct default values', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    expect(screen.getByLabelText('Floor depth (mm)')).toHaveValue(5)
    expect(screen.getByLabelText('Step-down (mm)')).toHaveValue(1)
    expect(screen.getByLabelText('Stepover (%)')).toHaveValue(50)
  })

  it('resets input values when operationId changes to a different pocket operation', async () => {
    const OTHER_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000003'
    const OTHER_OP: Operation = {
      id: OTHER_OP_ID,
      name: 'Finish Pocket',
      enabled: true,
      toolId: TOOL_ID_A,
      type: 'pocket',
      params: { depth: 20.0, stepdown: 0.5, stepoverPercent: 30.0 },
    }

    vi.mocked(opsApi.listOperations)
      .mockResolvedValueOnce([POCKET_OP])
      .mockResolvedValue([OTHER_OP])

    const { rerender } = render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toHaveValue(5))

    rerender(<OperationEditorForm operationId={OTHER_OP_ID} />)
    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toHaveValue(20))
    expect(screen.getByLabelText('Step-down (mm)')).toHaveValue(0.5)
    expect(screen.getByLabelText('Stepover (%)')).toHaveValue(30)
  })
})

// ── Save on tool change ───────────────────────────────────────────────────────

describe('OperationEditorForm — tool change saves', () => {
  it('calls editOperation with new toolId when tool select changes', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue({ ...POCKET_OP, toolId: TOOL_ID_B })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox')).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox'), { target: { value: TOOL_ID_B } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ toolId: TOOL_ID_B }),
    ))
  })

  it('refreshes snapshot after tool change', async () => {
    const updatedSnapshot = { ...SNAPSHOT_BASE, projectName: 'After Tool Change' }
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue({ ...POCKET_OP, toolId: TOOL_ID_B })
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(updatedSnapshot)

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox')).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox'), { target: { value: TOOL_ID_B } })

    await waitFor(() => expect(fileApi.getProjectSnapshot).toHaveBeenCalled())
    expect(useProjectStore.getState().snapshot?.projectName).toBe('After Tool Change')
  })
})

// ── Save on input blur ────────────────────────────────────────────────────────

describe('OperationEditorForm — input blur saves', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue(POCKET_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('calls editOperation with updated depth on blur', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Floor depth (mm)'), { target: { value: '8' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ depth: 8 }) }),
    ))
  })

  it('calls editOperation with updated stepdown on blur', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Step-down (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Step-down (mm)'), { target: { value: '2' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepdown: 2 }) }),
    ))
  })

  it('calls editOperation with updated stepoverPercent on blur', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepover (%)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Stepover (%)'), { target: { value: '40' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepoverPercent: 40 }) }),
    ))
  })
})

// ── Error handling ────────────────────────────────────────────────────────────

describe('OperationEditorForm — error handling', () => {
  it('pushes notification when listOperations rejects', async () => {
    vi.mocked(opsApi.listOperations).mockRejectedValue({ kind: 'NotFound', message: 'Op not found' })

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain('Op not found')
    )
  })

  it('pushes notification when editOperation rejects', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    vi.mocked(opsApi.editOperation).mockRejectedValue({ kind: 'InvalidInput', message: 'Bad tool' })

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox')).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox'), { target: { value: TOOL_ID_B } })

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain('Bad tool')
    )
  })
})
