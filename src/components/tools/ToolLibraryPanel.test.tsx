/**
 * Tests for ToolLibraryPanel.tsx — tool list, add/edit/delete, and error
 * handling.
 *
 * The tools and file API modules are mocked so tests run in jsdom without a
 * real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ToolLibraryPanel } from './ToolLibraryPanel'
import { useProjectStore } from '../../store/projectStore'
import type { Tool, ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/tools', () => ({
  addTool: vi.fn(),
  editTool: vi.fn(),
  deleteTool: vi.fn(),
  listTools: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

const toolsApi = await import('../../api/tools')
const fileApi = await import('../../api/file')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SNAPSHOT_WITH_TOOLS: ProjectSnapshot = {
  tools: [
    { id: 'tool-1', name: '6mm Flat', toolType: 'flat_endmill' },
    { id: 'tool-2', name: '3mm Ball', toolType: 'ball_nose' },
  ],
  stock: null,
  operations: [],
  wcs: [],
  projectName: 'test',
  modelPath: null,
  modelChecksum: null,
  modifiedAt: '',
  projectIsOpen: false,
}

const FULL_TOOL: Tool = {
  id: 'tool-1',
  name: '6mm Flat',
  type: 'flat_endmill',
  material: 'HSS',
  diameter: 6,
  fluteCount: 4,
  defaultSpindleSpeed: 12000,
  defaultFeedRate: 1500,
  cuttingLength: 18,
  shankDiameter: 6,
  overallLength: 54,
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
})

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('ToolLibraryPanel — rendering', () => {
  it('renders tool names and types from the store', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    render(<ToolLibraryPanel />)
    expect(screen.getByText('6mm Flat')).toBeInTheDocument()
    expect(screen.getByText('3mm Ball')).toBeInTheDocument()
    expect(screen.getByText('flat_endmill')).toBeInTheDocument()
    expect(screen.getByText('ball_nose')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Add Tool/i })).toBeInTheDocument()
  })

  it('renders no tool rows when snapshot is null', () => {
    render(<ToolLibraryPanel />)
    expect(screen.queryByText('6mm Flat')).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /Add Tool/i })).toBeInTheDocument()
  })
})

// ── Add form ──────────────────────────────────────────────────────────────────

describe('ToolLibraryPanel — add form', () => {
  it('opens add form when "Add Tool" is clicked', () => {
    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: /Add Tool/i }))
    expect(screen.getByLabelText('Name')).toBeInTheDocument()
    expect(screen.getByLabelText('Type')).toBeInTheDocument()
    expect(screen.getByLabelText('Material')).toBeInTheDocument()
    expect(screen.getByLabelText('Diameter (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Flute count')).toBeInTheDocument()
  })

  it('submits add form and updates store', async () => {
    vi.mocked(toolsApi.addTool).mockResolvedValue(FULL_TOOL)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_TOOLS)

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: /Add Tool/i }))

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: '6mm Flat' } })
    fireEvent.change(screen.getByLabelText('Type'), { target: { value: 'flat_endmill' } })
    fireEvent.change(screen.getByLabelText('Material'), { target: { value: 'HSS' } })
    fireEvent.change(screen.getByLabelText('Diameter (mm)'), { target: { value: '6' } })
    fireEvent.change(screen.getByLabelText('Flute count'), { target: { value: '4' } })

    fireEvent.click(screen.getByRole('button', { name: 'Add' }))

    await waitFor(() => expect(toolsApi.addTool).toHaveBeenCalledWith({
      name: expect.any(String),
      type: 'flat_endmill',
      material: expect.any(String),
      diameter: 6,
      fluteCount: 4,
      defaultSpindleSpeed: undefined,
      defaultFeedRate: undefined,
    }))
    expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
    expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT_WITH_TOOLS)
  })

  it('cancels add form without calling addTool', () => {
    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: /Add Tool/i }))
    expect(screen.getByLabelText('Name')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))
    expect(screen.queryByLabelText('Name')).not.toBeInTheDocument()
    expect(toolsApi.addTool).not.toHaveBeenCalled()
  })
})

// ── Edit ──────────────────────────────────────────────────────────────────────

describe('ToolLibraryPanel — edit', () => {
  it('fetches tool data and pre-populates form on edit click', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.listTools).mockResolvedValue([FULL_TOOL])

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Edit 6mm Flat' }))

    await waitFor(() => expect(toolsApi.listTools).toHaveBeenCalled())
    expect((screen.getByLabelText('Diameter (mm)') as HTMLInputElement).value).toBe('6')
    expect((screen.getByLabelText('Material') as HTMLInputElement).value).toBe('HSS')
    expect((screen.getByLabelText('Flute count') as HTMLInputElement).value).toBe('4')
  })

  it('submits edit form with correct arguments', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.listTools).mockResolvedValue([FULL_TOOL])
    vi.mocked(toolsApi.editTool).mockResolvedValue(FULL_TOOL)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_TOOLS)

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Edit 6mm Flat' }))

    await waitFor(() => expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))

    await waitFor(() => expect(toolsApi.editTool).toHaveBeenCalledWith('tool-1', {
      name: '6mm Flat',
      type: 'flat_endmill',
      material: 'HSS',
      diameter: 6,
      fluteCount: 4,
      defaultSpindleSpeed: 12000,
      defaultFeedRate: 1500,
    }))
    expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
    expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT_WITH_TOOLS)
  })
})

// ── Delete ────────────────────────────────────────────────────────────────────

describe('ToolLibraryPanel — delete', () => {
  it('calls deleteTool and refreshes snapshot', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.deleteTool).mockResolvedValue(undefined)
    vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_TOOLS)

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Delete 6mm Flat' }))

    await waitFor(() => expect(toolsApi.deleteTool).toHaveBeenCalledWith('tool-1'))
    expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
  })
})

// ── Error handling ────────────────────────────────────────────────────────────

describe('ToolLibraryPanel — error handling', () => {
  it('pushes notification when addTool rejects', async () => {
    vi.mocked(toolsApi.addTool).mockRejectedValue({ kind: 'SaveFailed', message: 'disk full' })

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: /Add Tool/i }))
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'Test' } })
    fireEvent.change(screen.getByLabelText('Material'), { target: { value: 'HSS' } })
    fireEvent.change(screen.getByLabelText('Diameter (mm)'), { target: { value: '6' } })
    fireEvent.change(screen.getByLabelText('Flute count'), { target: { value: '4' } })
    fireEvent.click(screen.getByRole('button', { name: 'Add' }))

    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('disk full'))
  })

  it('pushes notification when editTool rejects', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.listTools).mockResolvedValue([FULL_TOOL])
    vi.mocked(toolsApi.editTool).mockRejectedValue({ kind: 'SaveFailed', message: 'edit failed' })

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Edit 6mm Flat' }))

    await waitFor(() => expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument())
    fireEvent.click(screen.getByRole('button', { name: 'Save' }))

    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('edit failed'))
  })

  it('pushes notification when deleteTool rejects', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.deleteTool).mockRejectedValue({ kind: 'SaveFailed', message: 'delete failed' })

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Delete 6mm Flat' }))

    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('delete failed'))
  })

  it('pushes notification when listTools rejects on edit click', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_TOOLS, selectedOperationId: null, notifications: [] })
    vi.mocked(toolsApi.listTools).mockRejectedValue({ kind: 'NotFound', message: 'not found' })

    render(<ToolLibraryPanel />)
    fireEvent.click(screen.getByRole('button', { name: 'Edit 6mm Flat' }))

    await waitFor(() => expect(useProjectStore.getState().notifications).toContain('not found'))
  })
})
