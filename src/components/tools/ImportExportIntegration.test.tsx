/**
 * Tests for import/export flows in ToolEditorWindow — import-from-library
 * picker, export-to-library button, and cross-window refresh wiring.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
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

const SNAPSHOT_OPEN: ProjectSnapshot = {
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

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
  useGlobalToolStore.setState({ globalTools: [] })
})

// ── Import from Library ─────────────────────────────────────────────────────

describe('ToolEditorWindow — import from library', () => {
  it('shows "Import from Library" button in project context', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))

    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())
    expect(screen.getByRole('button', { name: /import from library/i })).toBeInTheDocument()
  })

  it('does not show "Import from Library" button in global context', () => {
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    render(<ToolEditorWindow />)
    expect(screen.queryByRole('button', { name: /import from library/i })).not.toBeInTheDocument()
  })

  it('opens import picker when "Import from Library" is clicked', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))
    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /import from library/i }))

    // Picker should show global library tools with checkboxes
    expect(screen.getByText('8mm Global Flat')).toBeInTheDocument()
    expect(screen.getByText('4mm Global Ball')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /import selected/i })).toBeInTheDocument()
  })

  it('calls importFromLibrary for each selected tool and refreshes', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    const importedTool: Tool = {
      id: 'pt-new',
      name: '8mm Global Flat',
      type: 'flat_endmill',
      material: 'Carbide',
      diameter: 8,
      fluteCount: 4,
      cuttingLength: 20,
      shankDiameter: 8,
      overallLength: 60,
    }
    vi.mocked(globalToolsApi.importFromLibrary).mockResolvedValue(importedTool)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))
    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /import from library/i }))

    // Select first tool and import
    fireEvent.click(screen.getAllByRole('checkbox')[0])

    // After import, expect the list to be refreshed
    vi.mocked(toolsApi.listTools).mockResolvedValue([...PROJECT_TOOLS, importedTool])
    fireEvent.click(screen.getByRole('button', { name: /import selected/i }))

    await waitFor(() => {
      expect(globalToolsApi.importFromLibrary).toHaveBeenCalledWith('gt-1')
    })
    // Project tools should be re-fetched
    await waitFor(() => {
      expect(toolsApi.listTools).toHaveBeenCalledTimes(2) // initial + after import
    })
  })
})

// ── Export to Library ───────────────────────────────────────────────────────

describe('ToolEditorWindow — export to library', () => {
  it('shows export button on each tool row in project context', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))

    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())
    expect(screen.getByRole('button', { name: /export 6mm project flat/i })).toBeInTheDocument()
  })

  it('does not show export button in global context', () => {
    useGlobalToolStore.setState({ globalTools: GLOBAL_TOOLS })
    render(<ToolEditorWindow />)
    expect(screen.queryByRole('button', { name: /export/i })).not.toBeInTheDocument()
  })

  it('calls exportToLibrary with tool ID and refreshes global tools', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    useGlobalToolStore.setState({ globalTools: [] })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)

    const exportedTool: Tool = {
      id: 'gt-new',
      name: '6mm Project Flat',
      type: 'flat_endmill',
      material: 'HSS',
      diameter: 6,
      fluteCount: 4,
      cuttingLength: 18,
      shankDiameter: 6,
      overallLength: 54,
    }
    vi.mocked(globalToolsApi.exportToLibrary).mockResolvedValue(exportedTool)
    vi.mocked(globalToolsApi.listGlobalTools).mockResolvedValue([exportedTool])

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))
    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /export 6mm project flat/i }))

    await waitFor(() => {
      expect(globalToolsApi.exportToLibrary).toHaveBeenCalledWith('pt-1')
    })
    await waitFor(() => {
      expect(globalToolsApi.listGlobalTools).toHaveBeenCalled()
    })
    // Global store should be updated
    expect(useGlobalToolStore.getState().globalTools).toEqual([exportedTool])
  })

  it('shows notification on export success', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_OPEN })
    vi.mocked(toolsApi.listTools).mockResolvedValue(PROJECT_TOOLS)
    vi.mocked(globalToolsApi.exportToLibrary).mockResolvedValue({
      ...PROJECT_TOOLS[0],
      id: 'gt-new',
    })
    vi.mocked(globalToolsApi.listGlobalTools).mockResolvedValue([])

    render(<ToolEditorWindow />)
    fireEvent.click(screen.getByRole('tab', { name: /project tools/i }))
    await waitFor(() => expect(screen.getByText('6mm Project Flat')).toBeInTheDocument())

    fireEvent.click(screen.getByRole('button', { name: /export 6mm project flat/i }))

    await waitFor(() => {
      const notifications = useProjectStore.getState().notifications
      expect(notifications.some((n) => /exported/i.test(n))).toBe(true)
    })
  })
})
