/**
 * Tests for ToolEditorWindow.tsx — context tabs, disabled project tab,
 * data wiring, and delete dispatch.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { ToolEditorWindow } from './ToolEditorWindow'
import { useProjectStore } from '../../store/projectStore'
import { useGlobalToolStore } from '../../store/globalToolStore'
import type { Tool, ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/tools', () => ({
  addTool: vi.fn(),
  editTool: vi.fn(),
  deleteTool: vi.fn(),
  listTools: vi.fn(),
}))

vi.mock('../../api/globalTools', () => ({
  listGlobalTools: vi.fn(),
  addGlobalTool: vi.fn(),
  editGlobalTool: vi.fn(),
  deleteGlobalTool: vi.fn(),
  importFromLibrary: vi.fn(),
  exportToLibrary: vi.fn(),
  isProjectOpen: vi.fn(),
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn())),
}))

const toolsApi = await import('../../api/tools')
const globalToolsApi = await import('../../api/globalTools')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const GLOBAL_TOOLS: Tool[] = [
  {
    id: 'gt-1',
    name: '8mm Global Flat',
    type: 'flat_endmill',
    material: 'Carbide',
    diameter: 8,
    fluteCount: 4,
    cuttingLength: 20,
    shankDiameter: 8,
    overallLength: 60,
  },
  {
    id: 'gt-2',
    name: '4mm Global Ball',
    type: 'ball_nose',
    material: 'HSS',
    diameter: 4,
    fluteCount: 2,
    cuttingLength: 12,
    shankDiameter: 4,
    overallLength: 40,
  },
]

const PROJECT_TOOLS: Tool[] = [
  {
    id: 'pt-1',
    name: '6mm Project Flat',
    type: 'flat_endmill',
    material: 'HSS',
    diameter: 6,
    fluteCount: 4,
    cuttingLength: 18,
    shankDiameter: 6,
    overallLength: 54,
  },
]

const SNAPSHOT_PROJECT_OPEN: ProjectSnapshot = {
  tools: [{ id: 'pt-1', name: '6mm Project Flat', toolType: 'flat_endmill' }],
  stock: null,
  operations: [],
  wcs: [],
  projectName: 'test',
  modelPath: null,
  modelChecksum: null,
  modifiedAt: '',
  projectIsOpen: true,
}

const SNAPSHOT_PROJECT_CLOSED: ProjectSnapshot = {
  tools: [],
  stock: null,
  operations: [],
  wcs: [],
  projectName: '',
  modelPath: null,
  modelChecksum: null,
  modifiedAt: '',
  projectIsOpen: false,
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
  useGlobalToolStore.setState({ globalTools: [] })
})

// ── Context tabs ─────────────────────────────────────────────────────────────

describe('ToolEditorWindow — context tabs', () => {
  it('renders both context tabs', () => {
    render(<ToolEditorWindow />)
    expect(screen.getByRole('tab', { name: /global library/i })).toBeInTheDocument()
    expect(screen.getByRole('tab', { name: /project tools/i })).toBeInTheDocument()
  })

  it('defaults to global library tab active', () => {
    render(<ToolEditorWindow />)
    const globalTab = screen.getByRole('tab', { name: /global library/i })
    expect(globalTab).toHaveAttribute('aria-selected', 'true')
  })

  it('disables project tab when projectIsOpen is false in snapshot', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_CLOSED })
    render(<ToolEditorWindow />)
    const projectTab = screen.getByRole('tab', { name: /project tools/i })
    expect(projectTab).toHaveAttribute('aria-disabled', 'true')
  })

  it('enables project tab when projectIsOpen is true', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_OPEN })
    render(<ToolEditorWindow />)
    const projectTab = screen.getByRole('tab', { name: /project tools/i })
    expect(projectTab).not.toHaveAttribute('aria-disabled', 'true')
  })

  it('shows disabled message when clicking disabled project tab', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_CLOSED })
    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))
    // Should NOT switch to project context
    expect(screen.getByRole('tab', { name: /global library/i })).toHaveAttribute('aria-selected', 'true')
  })

  it('shows message when no project is open and project tab is disabled', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_CLOSED })
    render(<ToolEditorWindow />)
    expect(screen.getByText(/open a project to manage project tools/i)).toBeInTheDocument()
  })
})

// ── Global context ───────────────────────────────────────────────────────────

describe('ToolEditorWindow — global context', () => {
  it('shows global tools from the store', () => {
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    render(<ToolEditorWindow />)
    expect(screen.getByText('8mm Global Flat')).toBeInTheDocument()
    expect(screen.getByText('4mm Global Ball')).toBeInTheDocument()
  })

  it('calls deleteGlobalTool and refreshes the store', async () => {
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    vi.mocked(globalToolsApi.deleteGlobalTool).mockResolvedValue(undefined)
    vi.mocked(globalToolsApi.listGlobalTools).mockResolvedValue([GLOBAL_TOOLS[1]])

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('button', { name: 'Delete 8mm Global Flat' }))

    await waitFor(() => expect(globalToolsApi.deleteGlobalTool).toHaveBeenCalledWith('gt-1'))
    await waitFor(() => expect(globalToolsApi.listGlobalTools).toHaveBeenCalled())
    expect(useGlobalToolStore.getState().globalTools).toEqual([GLOBAL_TOOLS[1]])
  })
})

// ── Project context ──────────────────────────────────────────────────────────

describe('ToolEditorWindow — project context', () => {
  it('fetches and shows project tools when switching to project tab', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_OPEN })
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))

    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())
    expect(toolsApi.listTools).toHaveBeenCalled()
  })

  it('switches to global context when project closes while on project tab', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_OPEN })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))

    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    // Project closes — snapshot updates with projectIsOpen: false
    act(() => useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_CLOSED }))

    await waitFor(() => {
      expect(screen.getByRole('tab', { name: /global library/i })).toHaveAttribute('aria-selected', 'true')
    })
    // Stale project tools should be cleared
    expect(screen.queryByText('6mm Project Flat')).not.toBeInTheDocument()
  })

  it('calls deleteTool when delete is clicked in project context', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_PROJECT_OPEN })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)
    vi.mocked(toolsApi.deleteTool).mockResolvedValue(undefined)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))

    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    // After delete, listTools is called again to refresh
    vi.mocked(toolsApi.listTools).mockResolvedValue([])
    fireEvent.click(screen.getByRole('button', { name: 'Delete 6mm Project Flat' }))

    await waitFor(() => expect(toolsApi.deleteTool).toHaveBeenCalledWith('pt-1'))
  })
})

// ── Edit selection ───────────────────────────────────────────────────────────

describe('ToolEditorWindow — edit selection', () => {
  it('shows editing placeholder when edit is clicked', async () => {
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    render(<ToolEditorWindow />)

    fireEvent.click(screen.getByRole('button', { name: 'Edit 8mm Global Flat' }))
    expect(screen.getByText(/editing tool/i)).toBeInTheDocument()
  })
})
