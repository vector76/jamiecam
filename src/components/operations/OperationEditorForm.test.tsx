/**
 * Tests for OperationEditorForm.tsx — renders forms for editing pocket and
 * profile operations, handles null/unknown states, and calls the API on field change.
 *
 * API modules are mocked so tests run in jsdom without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { OperationEditorForm } from './OperationEditorForm'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import type { Operation, ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/operations', () => ({
  listOperations: vi.fn(),
  editOperation: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

vi.mock('../../api/geometry', () => ({
  getModelFaces: vi.fn(),
  detectHoles: vi.fn(),
}))

vi.mock('../../api/feeds', () => ({
  listMaterials: vi.fn(),
  lookupFeeds: vi.fn(),
}))

const opsApi = await import('../../api/operations')
const fileApi = await import('../../api/file')
const geoApi = await import('../../api/geometry')
const feedsApi = await import('../../api/feeds')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const TOOL_ID_A = 'aaaaaaaa-0000-0000-0000-000000000001'
const TOOL_ID_B = 'aaaaaaaa-0000-0000-0000-000000000002'
const POCKET_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000001'
const PROFILE_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000002'
const DRILL_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000003'
const ZLEVEL_ROUGHING_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000004'

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

const DRILL_OP: Operation = {
  id: DRILL_OP_ID,
  name: 'Drill',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'drill',
  params: { depth: 10.0, peckDepth: 3.0, points: [] },
}

const ZLEVEL_ROUGHING_OP: Operation = {
  id: ZLEVEL_ROUGHING_OP_ID,
  name: 'Z-Level Roughing',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'z_level_roughing',
  params: { depth: 5.0, stepdown: 1.0, stepover: 0.5 },
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
  useViewportStore.setState({
    selectionMode: false,
    selectedFaceFingerprints: [],
    faceDescriptors: [],
    hoveredFaceIdx: null,
  })
  vi.mocked(geoApi.getModelFaces).mockResolvedValue([])
  vi.mocked(feedsApi.listMaterials).mockResolvedValue([])
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

  it('spindle speed and feed rate override inputs are present on the profile form', async () => {
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Spindle speed override (RPM)')).toBeInTheDocument())
    expect(screen.getByLabelText('Feed rate override (mm/min)')).toBeInTheDocument()
  })

  it('renders all five entry motion fields for a profile operation', async () => {
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument())
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (°)')).toBeInTheDocument()
  })

  it('entry motion fields are empty by default when params have no values', async () => {
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument())
    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Ramp entry angle (°)')).toHaveValue(null)
  })

  it('arc lead-in radius blur with a value calls editOperation with arcLeadInRadius set', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '2.5' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ arcLeadInRadius: 2.5 }) }),
    ))
  })

  it('arc lead-in radius blur with blank sends null', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ arcLeadInRadius: null }) }),
    ))
  })

  it('profile stepdown blur with blank sends null', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(PROFILE_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Step-down (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Step-down (mm)'), { target: { value: '' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PROFILE_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepdown: null }) }),
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

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
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

  it('spindle speed and feed rate override inputs are present on the pocket form', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Spindle speed override (RPM)')).toBeInTheDocument())
    expect(screen.getByLabelText('Feed rate override (mm/min)')).toBeInTheDocument()
  })

  it('renders all five entry motion fields for a pocket operation', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument())
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (°)')).toBeInTheDocument()
  })

  it('arc lead-out radius blur with a value calls editOperation with arcLeadOutRadius set', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(POCKET_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '3' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ arcLeadOutRadius: 3 }) }),
    ))
  })

  it('arc lead-out radius blur with blank sends null', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(POCKET_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ arcLeadOutRadius: null }) }),
    ))
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

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox', { name: 'Tool' }), { target: { value: TOOL_ID_B } })

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

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox', { name: 'Tool' }), { target: { value: TOOL_ID_B } })

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

// ── Drill form rendering ──────────────────────────────────────────────────────

describe('OperationEditorForm — drill form', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
  })

  it('drill form renders tool select, depth, and peck depth inputs', async () => {
    render(<OperationEditorForm operationId={DRILL_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Peck depth (mm)')).toBeInTheDocument()
  })

  it('Add point button appends a new row to the points list', async () => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(DRILL_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Add point')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Add point'))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      DRILL_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ points: [{ x: 0, y: 0 }] }) }),
    ))
  })

  it('spindle speed and feed rate override inputs are present on the drill form', async () => {
    render(<OperationEditorForm operationId={DRILL_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Spindle speed override (RPM)')).toBeInTheDocument())
    expect(screen.getByLabelText('Feed rate override (mm/min)')).toBeInTheDocument()
  })

  it('Remove button removes the correct row from the points list', async () => {
    const DRILL_OP_WITH_POINTS: Operation = {
      ...DRILL_OP,
      params: { depth: 10.0, peckDepth: 3.0, points: [{ x: 10, y: 20 }, { x: 30, y: 40 }] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP_WITH_POINTS])
    vi.mocked(opsApi.editOperation).mockResolvedValue(DRILL_OP_WITH_POINTS)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)

    await waitFor(() => {
      const removeButtons = screen.getAllByText('Remove')
      expect(removeButtons).toHaveLength(2)
    })

    fireEvent.click(screen.getAllByText('Remove')[0])

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      DRILL_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ points: [{ x: 30, y: 40 }] }) }),
    ))
  })
})

// ── Geometry section ──────────────────────────────────────────────────────────

describe('OperationEditorForm — geometry section', () => {
  beforeEach(() => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(POCKET_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('Select Faces button appears for pocket operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
  })

  it('Select Faces button appears for profile operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([PROFILE_OP])
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
  })

  it('Select Faces button does not appear for drill operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument())
    expect(screen.queryByText('Select Faces')).not.toBeInTheDocument()
  })

  it('Clicking Select Faces calls getModelFaces and sets selectionMode true', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(geoApi.getModelFaces).toHaveBeenCalled())
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('Clicking Select Faces pre-populates fingerprints from saved geometry', async () => {
    const POCKET_WITH_GEO: Operation = {
      ...POCKET_OP,
      params: { depth: 5.0, stepdown: 1.0, stepoverPercent: 50.0, geometry: ['fp-a', 'fp-b'] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_WITH_GEO])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-a', 'fp-b'])
  })

  it('While selectionMode is true, button text is Done Selecting', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    expect(screen.queryByText('Select Faces')).not.toBeInTheDocument()
  })

  it('Clicking Done Selecting sets selectionMode false and calls editOperation with fingerprints', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-x', 'fp-y'] })
    fireEvent.click(screen.getByText('Done Selecting'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(false))
    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: ['fp-x', 'fp-y'] }) }),
    ))
  })

  it('Clear button appears when operation has saved fingerprints', async () => {
    const POCKET_WITH_GEO: Operation = {
      ...POCKET_OP,
      params: { depth: 5.0, stepdown: 1.0, stepoverPercent: 50.0, geometry: ['fp-a'] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_WITH_GEO])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Clear')).toBeInTheDocument())
  })

  it('Clicking Clear calls editOperation with geometry null', async () => {
    const POCKET_WITH_GEO: Operation = {
      ...POCKET_OP,
      params: { depth: 5.0, stepdown: 1.0, stepoverPercent: 50.0, geometry: ['fp-a'] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_WITH_GEO])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Clear')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Clear'))
    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      POCKET_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: null }) }),
    ))
  })
})

// ── Z-Level Roughing form ─────────────────────────────────────────────────────

describe('OperationEditorForm — z_level_roughing branch', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([ZLEVEL_ROUGHING_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue(ZLEVEL_ROUGHING_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('renders tool selector, Depth, Stepdown, Stepover inputs, geometry section', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Stepdown (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Stepover (%)')).toBeInTheDocument()
    expect(screen.getByText('Select Faces')).toBeInTheDocument()
  })

  it('depth input blur calls editOperation with correct params.depth', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Depth (mm)'), { target: { value: '8' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ZLEVEL_ROUGHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ depth: 8 }) }),
    ))
  })

  it('stepdown input blur calls editOperation with correct params.stepdown', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepdown (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Stepdown (mm)'), { target: { value: '0.5' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ZLEVEL_ROUGHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepdown: 0.5 }) }),
    ))
  })

  it('stepover input displays value as percentage (0.5 → 50)', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepover (%)')).toHaveValue(50))
  })

  it('stepover input blur with 40 saves params.stepover as 0.4', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepover (%)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Stepover (%)'), { target: { value: '40' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ZLEVEL_ROUGHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepover: 0.4 }) }),
    ))
  })

  it('Select Faces button enters selection mode', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('Done Selecting saves geometry fingerprints', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-1', 'fp-2'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ZLEVEL_ROUGHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: ['fp-1', 'fp-2'] }) }),
    ))
  })

  it('Clear button saves geometry as null', async () => {
    const OP_WITH_GEO: Operation = {
      ...ZLEVEL_ROUGHING_OP,
      params: { depth: 5.0, stepdown: 1.0, stepover: 0.5, geometry: ['fp-a'] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([OP_WITH_GEO])

    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Clear')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Clear'))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ZLEVEL_ROUGHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: null }) }),
    ))
  })

  it('Calculate button is disabled when stock is null', async () => {
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).toBeDisabled())
  })

  it('Calculate button is enabled when stock is defined', async () => {
    useProjectStore.setState({
      snapshot: {
        ...SNAPSHOT_BASE,
        stock: { type: 'box', origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 100, height: 50 },
      },
    })
    render(<OperationEditorForm operationId={ZLEVEL_ROUGHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).not.toBeDisabled())
  })
})

// ── Detect Holes ──────────────────────────────────────────────────────────────

describe('OperationEditorForm — detect holes', () => {
  beforeEach(() => {
    vi.mocked(opsApi.editOperation).mockResolvedValue(DRILL_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('Detect Holes button renders only for drill operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
  })

  it('Detect Holes button does not render for pocket operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)
    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    expect(screen.queryByText('Detect Holes')).not.toBeInTheDocument()
  })

  it('Detect Holes button does not render for profile operations', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([PROFILE_OP])
    render(<OperationEditorForm operationId={PROFILE_OP_ID} />)
    await waitFor(() => expect(screen.getByLabelText('Floor depth (mm)')).toBeInTheDocument())
    expect(screen.queryByText('Detect Holes')).not.toBeInTheDocument()
  })

  it('clicking Detect Holes calls detectHoles API and populates points', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
    vi.mocked(geoApi.detectHoles).mockResolvedValue([
      { centerX: 10, centerY: 20, radius: 3, depth: 5, isThrough: false },
      { centerX: 30, centerY: 40, radius: 3, depth: 5, isThrough: true },
    ])

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Detect Holes'))

    await waitFor(() => expect(geoApi.detectHoles).toHaveBeenCalled())
    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      DRILL_OP_ID,
      expect.objectContaining({
        params: expect.objectContaining({
          points: [{ x: 10, y: 20 }, { x: 30, y: 40 }],
        }),
      }),
    ))
  })

  it('shows confirmation dialog when existing points would be replaced', async () => {
    const DRILL_WITH_POINTS: Operation = {
      ...DRILL_OP,
      params: { depth: 10.0, peckDepth: 3.0, points: [{ x: 1, y: 2 }] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_WITH_POINTS])
    vi.mocked(geoApi.detectHoles).mockResolvedValue([
      { centerX: 10, centerY: 20, radius: 3, depth: 5, isThrough: false },
    ])
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(true)

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Detect Holes'))

    await waitFor(() => expect(confirmSpy).toHaveBeenCalledWith('Replace existing drill points with 1 detected holes?'))
    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalled())
    confirmSpy.mockRestore()
  })

  it('does not replace points when user cancels confirmation', async () => {
    const DRILL_WITH_POINTS: Operation = {
      ...DRILL_OP,
      params: { depth: 10.0, peckDepth: 3.0, points: [{ x: 1, y: 2 }] },
    }
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_WITH_POINTS])
    vi.mocked(geoApi.detectHoles).mockResolvedValue([
      { centerX: 10, centerY: 20, radius: 3, depth: 5, isThrough: false },
    ])
    const confirmSpy = vi.spyOn(window, 'confirm').mockReturnValue(false)

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Detect Holes'))

    await waitFor(() => expect(confirmSpy).toHaveBeenCalled())
    expect(opsApi.editOperation).not.toHaveBeenCalled()
    confirmSpy.mockRestore()
  })

  it('shows notification when no holes are detected', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
    vi.mocked(geoApi.detectHoles).mockResolvedValue([])

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Detect Holes'))

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain('No holes detected')
    )
    expect(opsApi.editOperation).not.toHaveBeenCalled()
  })

  it('shows notification when detectHoles API rejects', async () => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([DRILL_OP])
    vi.mocked(geoApi.detectHoles).mockRejectedValue({ kind: 'BackendError', message: 'No model loaded' })

    render(<OperationEditorForm operationId={DRILL_OP_ID} />)
    await waitFor(() => expect(screen.getByText('Detect Holes')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Detect Holes'))

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain('No model loaded')
    )
  })
})

// ── Adaptive Clearing form ─────────────────────────────────────────────────

const ADAPTIVE_CLEARING_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000005'

const ADAPTIVE_CLEARING_OP: Operation = {
  id: ADAPTIVE_CLEARING_OP_ID,
  name: 'Adaptive Clearing',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'adaptive_clearing',
  params: { depth: 5.0, stepdown: 1.0, optimalLoad: 0.25, stepoverPercent: 50 },
}

describe('OperationEditorForm — adaptive_clearing branch', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([ADAPTIVE_CLEARING_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue(ADAPTIVE_CLEARING_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('renders tool selector, Depth, Stepdown, Optimal load, Stepover, and entry motion fields', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Stepdown (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Optimal load (%)')).toBeInTheDocument()
    expect(screen.getByLabelText('Stepover (%)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (°)')).toBeInTheDocument()
  })

  it('optimal load input displays value as percentage (0.25 → 25)', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Optimal load (%)')).toHaveValue(25))
  })

  it('optimal load input blur with 30 saves params.optimalLoad as 0.3', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Optimal load (%)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Optimal load (%)'), { target: { value: '30' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ADAPTIVE_CLEARING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ optimalLoad: 0.3 }) }),
    ))
  })

  it('depth input blur calls editOperation with correct params.depth', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Depth (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Depth (mm)'), { target: { value: '10' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ADAPTIVE_CLEARING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ depth: 10 }) }),
    ))
  })

  it('stepover input blur saves stepoverPercent directly (no division)', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepover (%)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Stepover (%)'), { target: { value: '40' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ADAPTIVE_CLEARING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepoverPercent: 40 }) }),
    ))
  })

  it('Select Faces button appears and enters selection mode', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('Done Selecting saves geometry fingerprints', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-1', 'fp-2'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      ADAPTIVE_CLEARING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: ['fp-1', 'fp-2'] }) }),
    ))
  })

  it('Calculate button is disabled when stock is null', async () => {
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).toBeDisabled())
  })

  it('Calculate button is enabled when stock is defined', async () => {
    useProjectStore.setState({
      snapshot: {
        ...SNAPSHOT_BASE,
        stock: { type: 'box', origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 100, height: 50 },
      },
    })
    render(<OperationEditorForm operationId={ADAPTIVE_CLEARING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).not.toBeDisabled())
  })
})

// ── Parallel Finishing form ───────────────────────────────────────────────────

const PARALLEL_FINISHING_OP_ID = 'bbbbbbbb-0000-0000-0000-000000000006'

const PARALLEL_FINISHING_OP: Operation = {
  id: PARALLEL_FINISHING_OP_ID,
  name: 'Parallel Finishing',
  enabled: true,
  toolId: TOOL_ID_A,
  type: 'parallelFinishing',
  params: { stepover: 1.0, directionAngleDeg: 0, allowance: 0.1 },
}

describe('OperationEditorForm — parallelFinishing branch', () => {
  beforeEach(() => {
    vi.mocked(opsApi.listOperations).mockResolvedValue([PARALLEL_FINISHING_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue(PARALLEL_FINISHING_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('renders tool selector, Stepover, Direction, Allowance, and entry motion fields', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    expect(screen.getByLabelText('Stepover (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Direction (°)')).toBeInTheDocument()
    expect(screen.getByLabelText('Allowance (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (°)')).toBeInTheDocument()
    expect(screen.getByText('Select Faces')).toBeInTheDocument()
  })

  it('renders spindle speed and feed rate override inputs', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Spindle speed override (RPM)')).toBeInTheDocument())
    expect(screen.getByLabelText('Feed rate override (mm/min)')).toBeInTheDocument()
  })

  it('stepover blur calls editOperation with updated stepover', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Stepover (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Stepover (mm)'), { target: { value: '0.5' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PARALLEL_FINISHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ stepover: 0.5 }) }),
    ))
  })

  it('direction blur calls editOperation with updated directionAngleDeg', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Direction (°)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Direction (°)'), { target: { value: '90' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PARALLEL_FINISHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ directionAngleDeg: 90 }) }),
    ))
  })

  it('allowance blur calls editOperation with updated allowance', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByLabelText('Allowance (mm)')).toBeInTheDocument())
    fireEvent.blur(screen.getByLabelText('Allowance (mm)'), { target: { value: '0.05' } })

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PARALLEL_FINISHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ allowance: 0.05 }) }),
    ))
  })

  it('Select Faces button enters selection mode', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('Done Selecting saves geometry fingerprints', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByText('Select Faces')).toBeInTheDocument())
    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    useViewportStore.setState({ selectedFaceFingerprints: ['fp-1', 'fp-2'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(opsApi.editOperation).toHaveBeenCalledWith(
      PARALLEL_FINISHING_OP_ID,
      expect.objectContaining({ params: expect.objectContaining({ geometry: ['fp-1', 'fp-2'] }) }),
    ))
  })

  it('Calculate button is disabled when stock is null', async () => {
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).toBeDisabled())
  })

  it('Calculate button is enabled when stock is defined', async () => {
    useProjectStore.setState({
      snapshot: {
        ...SNAPSHOT_BASE,
        stock: { type: 'box', origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 100, height: 50 },
      },
    })
    render(<OperationEditorForm operationId={PARALLEL_FINISHING_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('button', { name: 'Calculate' })).not.toBeDisabled())
  })
})

// ── Material selector ─────────────────────────────────────────────────────────

describe('OperationEditorForm — material selector', () => {
  beforeEach(() => {
    useProjectStore.setState({
      snapshot: {
        ...SNAPSHOT_BASE,
        tools: [
          { id: TOOL_ID_A, name: '10mm Flat', toolType: 'flat_endmill', material: 'carbide' },
        ],
      },
      notifications: [],
    })
    vi.mocked(feedsApi.listMaterials).mockResolvedValue([
      { id: 'aluminum-6061', displayName: 'Aluminum 6061' },
    ])
    vi.mocked(feedsApi.lookupFeeds).mockResolvedValue({
      spindleSpeedRpm: 12000,
      feedRateMmpm: 1500,
    })
    vi.mocked(opsApi.listOperations).mockResolvedValue([POCKET_OP])
    vi.mocked(opsApi.editOperation).mockResolvedValue(POCKET_OP)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_BASE)
  })

  it('selecting a material calls editOperation with spindle/feed overrides from lookup', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    // Wait until listMaterials resolves so the option exists in the DOM.
    // Without this, fireEvent.change on a select with no matching option silently
    // leaves select.value as '' and the handler receives an empty value.
    await waitFor(() => expect(screen.getByRole('option', { name: 'Aluminum 6061' })).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('-- Select material --'), {
      target: { value: 'aluminum-6061' },
    })

    await waitFor(() =>
      expect(opsApi.editOperation).toHaveBeenCalledWith(
        POCKET_OP_ID,
        expect.objectContaining({ workpieceMaterial: 'aluminum-6061' }),
      )
    )
    await waitFor(() =>
      expect(opsApi.editOperation).toHaveBeenCalledWith(
        POCKET_OP_ID,
        expect.objectContaining({ workpieceMaterial: 'aluminum-6061', spindleSpeedOverride: 12000, feedRateOverride: 1500 }),
      )
    )
  })

  it('blur on spindle override saves user-entered value after material pre-fill', async () => {
    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('option', { name: 'Aluminum 6061' })).toBeInTheDocument())

    // Select material to trigger pre-fill
    fireEvent.change(screen.getByDisplayValue('-- Select material --'), {
      target: { value: 'aluminum-6061' },
    })
    await waitFor(() =>
      expect(opsApi.editOperation).toHaveBeenCalledWith(
        POCKET_OP_ID,
        expect.objectContaining({ spindleSpeedOverride: 12000 }),
      )
    )

    vi.mocked(opsApi.editOperation).mockClear()

    // User manually sets a different spindle value
    fireEvent.blur(screen.getByLabelText('Spindle speed override (RPM)'), {
      target: { value: '15000' },
    })

    await waitFor(() =>
      expect(opsApi.editOperation).toHaveBeenCalledWith(
        POCKET_OP_ID,
        expect.objectContaining({ spindleSpeedOverride: 15000 }),
      )
    )
  })

  it('not-found response pushes a notification', async () => {
    vi.mocked(feedsApi.lookupFeeds).mockRejectedValue({ kind: 'NotFound' })

    render(<OperationEditorForm operationId={POCKET_OP_ID} />)

    await waitFor(() => expect(screen.getByRole('option', { name: 'Aluminum 6061' })).toBeInTheDocument())

    fireEvent.change(screen.getByDisplayValue('-- Select material --'), {
      target: { value: 'aluminum-6061' },
    })

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain(
        'No feed data found for this material/tool/category combination',
      )
    )
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

    await waitFor(() => expect(screen.getByRole('combobox', { name: 'Tool' })).toBeInTheDocument())
    fireEvent.change(screen.getByRole('combobox', { name: 'Tool' }), { target: { value: TOOL_ID_B } })

    await waitFor(() =>
      expect(useProjectStore.getState().notifications).toContain('Bad tool')
    )
  })
})
