/**
 * Tests for menuActions — shared file operation handlers.
 *
 * Validates that each handler calls the correct API functions, updates
 * the Zustand stores, and routes errors through pushNotification.
 */

import { useProjectStore } from '../store/projectStore'
import { useViewportStore } from '../store/viewportStore'
import type { MeshData, ProjectSnapshot, LineGeometryData } from '../api/types'

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}))

vi.mock('../api/file', () => ({
  openModel: vi.fn(),
  newProject: vi.fn(),
  saveProject: vi.fn(),
  saveProjectCurrent: vi.fn(),
  loadProject: vi.fn(),
  getProjectSnapshot: vi.fn(),
}))

vi.mock('../api/toolpath', () => ({
  getToolpathGeometry: vi.fn(),
}))

vi.mock('./unsavedGuard', () => ({
  checkUnsavedChanges: vi.fn(),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ setTitle: vi.fn() })),
}))

const { open, save } = await import('@tauri-apps/plugin-dialog')
const api = await import('../api/file')
const toolpathApi = await import('../api/toolpath')
const { checkUnsavedChanges } = await import('./unsavedGuard')

import {
  handleOpenModel,
  handleNewProject,
  handleSave,
  handleSaveAs,
  handleOpenProject,
  menuActionDispatch,
} from './menuActions'

// ── Fixtures ─────────────────────────────────────────────────────────────────

const MESH: MeshData = { vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0], normals: [0, 0, 1, 0, 0, 1, 0, 0, 1], indices: [0, 1, 2], faceGroups: [] }
const SNAPSHOT: ProjectSnapshot = { modelPath: '/models/part.step', modelChecksum: 'abc', projectName: 'Test', modifiedAt: '', tools: [], stock: null, wcs: [], operations: [], projectIsOpen: false, filePath: null, dirty: false, mode: '3d' }
const EMPTY_SNAPSHOT: ProjectSnapshot = { modelPath: null, modelChecksum: null, projectName: '', modifiedAt: '', tools: [], stock: null, wcs: [], operations: [], projectIsOpen: false, filePath: null, dirty: false, mode: '3d' }
const LINE_GEOMETRY: LineGeometryData = { positions: [0, 0, 0, 1, 0, 0], colours: [1, 0, 0, 1, 0, 0], types: [1] }
const OP_ID = 'op-1'
const SNAPSHOT_WITH_OP: ProjectSnapshot = { modelPath: null, modelChecksum: null, projectName: '', modifiedAt: '', tools: [], stock: null, wcs: [], operations: [{ id: OP_ID, name: 'Op 1', operationType: 'profile', enabled: true, needsRecalculate: false }], projectIsOpen: false, filePath: null, dirty: false, mode: '3d' }
const SNAPSHOT_WITH_STALE_OP: ProjectSnapshot = { modelPath: null, modelChecksum: null, projectName: '', modifiedAt: '', tools: [], stock: null, wcs: [], operations: [{ id: OP_ID, name: 'Op 1', operationType: 'profile', enabled: true, needsRecalculate: true }], projectIsOpen: false, filePath: null, dirty: false, mode: '3d' }
const SNAPSHOT_WITH_FILE_PATH: ProjectSnapshot = { ...SNAPSHOT, filePath: '/projects/my.jcam', dirty: true }

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
  useViewportStore.setState({ meshData: null, toolpathGeometry: null })
  vi.mocked(checkUnsavedChanges).mockResolvedValue(true)
})

// ── handleOpenModel ──────────────────────────────────────────────────────────

describe('handleOpenModel', () => {
  it('calls openModel with the selected path and updates stores', async () => {
    vi.mocked(open).mockResolvedValue('/models/part.step')
    vi.mocked(api.openModel).mockResolvedValue(MESH)
    vi.mocked(api.getProjectSnapshot).mockResolvedValue(SNAPSHOT)

    await handleOpenModel()

    expect(api.openModel).toHaveBeenCalledWith('/models/part.step')
    expect(useViewportStore.getState().meshData).toEqual(MESH)
    expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT)
  })

  it('does nothing when the dialog is cancelled', async () => {
    vi.mocked(open).mockResolvedValue(null)

    await handleOpenModel()

    expect(api.openModel).not.toHaveBeenCalled()
  })

  it('pushes a notification when openModel throws', async () => {
    vi.mocked(open).mockResolvedValue('/bad.step')
    vi.mocked(api.openModel).mockRejectedValue({ kind: 'GeometryImport', message: 'Failed to parse file' })

    await handleOpenModel()

    expect(useProjectStore.getState().notifications).toEqual(['Failed to parse file'])
  })
})

// ── handleNewProject ─────────────────────────────────────────────────────────

describe('handleNewProject', () => {
  it('calls newProject, clears meshData, and updates snapshot', async () => {
    useViewportStore.setState({ meshData: MESH })
    vi.mocked(api.newProject).mockResolvedValue(EMPTY_SNAPSHOT)

    await handleNewProject()

    expect(api.newProject).toHaveBeenCalledWith('3d')
    expect(useViewportStore.getState().meshData).toBeNull()
    expect(useProjectStore.getState().snapshot).toEqual(EMPTY_SNAPSHOT)
  })

  it('pushes a notification when newProject throws', async () => {
    vi.mocked(api.newProject).mockRejectedValue({ kind: 'Io', message: 'Disk full' })

    await handleNewProject()

    expect(useProjectStore.getState().notifications).toEqual(['Disk full'])
  })

  it('does nothing when the unsaved-changes guard returns false', async () => {
    vi.mocked(checkUnsavedChanges).mockResolvedValue(false)

    await handleNewProject()

    expect(api.newProject).not.toHaveBeenCalled()
  })
})

// ── handleSave ───────────────────────────────────────────────────────────────

describe('handleSave', () => {
  it('calls saveProjectCurrent when snapshot has a filePath', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_FILE_PATH })
    vi.mocked(api.saveProjectCurrent).mockResolvedValue(undefined)

    await handleSave()

    expect(api.saveProjectCurrent).toHaveBeenCalled()
    expect(save).not.toHaveBeenCalled()
  })

  it('falls through to Save As when snapshot has no filePath', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockResolvedValue(undefined)

    await handleSave()

    expect(api.saveProjectCurrent).not.toHaveBeenCalled()
    expect(save).toHaveBeenCalled()
    expect(api.saveProject).toHaveBeenCalledWith('/output/project.jcam')
  })

  it('falls through to Save As when snapshot is null', async () => {
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockResolvedValue(undefined)

    await handleSave()

    expect(api.saveProjectCurrent).not.toHaveBeenCalled()
    expect(save).toHaveBeenCalled()
  })

  it('pushes a notification when saveProjectCurrent throws', async () => {
    useProjectStore.setState({ snapshot: SNAPSHOT_WITH_FILE_PATH })
    vi.mocked(api.saveProjectCurrent).mockRejectedValue({ kind: 'Io', message: 'Write failed' })

    await handleSave()

    expect(useProjectStore.getState().notifications).toEqual(['Write failed'])
  })
})

// ── handleSaveAs ─────────────────────────────────────────────────────────────

describe('handleSaveAs', () => {
  it('calls saveProject with the chosen path', async () => {
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockResolvedValue(undefined)

    await handleSaveAs()

    expect(api.saveProject).toHaveBeenCalledWith('/output/project.jcam')
  })

  it('does nothing when the dialog is cancelled', async () => {
    vi.mocked(save).mockResolvedValue(null)

    await handleSaveAs()

    expect(api.saveProject).not.toHaveBeenCalled()
  })

  it('pushes a notification when saveProject throws', async () => {
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockRejectedValue({ kind: 'Io', message: 'Permission denied' })

    await handleSaveAs()

    expect(useProjectStore.getState().notifications).toEqual(['Permission denied'])
  })
})

// ── handleOpenProject ────────────────────────────────────────────────────────

describe('handleOpenProject', () => {
  it('calls loadProject and updates snapshot', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(EMPTY_SNAPSHOT)

    await handleOpenProject()

    expect(api.loadProject).toHaveBeenCalledWith('/projects/job.jcam')
    expect(useProjectStore.getState().snapshot).toEqual(EMPTY_SNAPSHOT)
  })

  it('reloads the model mesh when snapshot has a modelPath', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(SNAPSHOT)
    vi.mocked(api.openModel).mockResolvedValue(MESH)

    await handleOpenProject()

    expect(api.openModel).toHaveBeenCalledWith(SNAPSHOT.modelPath)
    expect(useViewportStore.getState().meshData).toEqual(MESH)
  })

  it('clears meshData when snapshot has no modelPath', async () => {
    useViewportStore.setState({ meshData: MESH })
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(EMPTY_SNAPSHOT)

    await handleOpenProject()

    expect(useViewportStore.getState().meshData).toBeNull()
  })

  it('does nothing when the dialog is cancelled', async () => {
    vi.mocked(open).mockResolvedValue(null)

    await handleOpenProject()

    expect(api.loadProject).not.toHaveBeenCalled()
  })

  it('does nothing when the unsaved-changes guard returns false', async () => {
    vi.mocked(checkUnsavedChanges).mockResolvedValue(false)

    await handleOpenProject()

    expect(open).not.toHaveBeenCalled()
    expect(api.loadProject).not.toHaveBeenCalled()
  })

  it('fetches toolpath geometry for non-stale operations', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(SNAPSHOT_WITH_OP)
    vi.mocked(toolpathApi.getToolpathGeometry).mockResolvedValue(LINE_GEOMETRY)

    await handleOpenProject()

    expect(toolpathApi.getToolpathGeometry).toHaveBeenCalledWith(OP_ID)
    expect(useViewportStore.getState().toolpathGeometry).toEqual(LINE_GEOMETRY)
  })

  it('skips toolpath geometry for stale operations', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(SNAPSHOT_WITH_STALE_OP)

    await handleOpenProject()

    expect(toolpathApi.getToolpathGeometry).not.toHaveBeenCalled()
  })

  it('pushes a notification when loadProject throws', async () => {
    vi.mocked(open).mockResolvedValue('/bad.jcam')
    vi.mocked(api.loadProject).mockRejectedValue({ kind: 'FileNotFound', message: 'File not found' })

    await handleOpenProject()

    expect(useProjectStore.getState().notifications).toEqual(['File not found'])
  })
})

// ── menuActionDispatch ──────────────────────────────────────────────────────

describe('menuActionDispatch', () => {
  it('maps every expected menu ID to a handler function', () => {
    const expectedIds = ['new-project', 'open-project', 'open-model', 'save', 'save-as']
    for (const id of expectedIds) {
      expect(menuActionDispatch[id], `missing handler for "${id}"`).toBeTypeOf('function')
    }
  })

  it('maps to the correct handler for each menu ID', () => {
    expect(menuActionDispatch['new-project']).toBe(handleNewProject)
    expect(menuActionDispatch['open-project']).toBe(handleOpenProject)
    expect(menuActionDispatch['open-model']).toBe(handleOpenModel)
    expect(menuActionDispatch['save']).toBe(handleSave)
    expect(menuActionDispatch['save-as']).toBe(handleSaveAs)
  })

  it('contains no extra entries beyond the expected menu IDs', () => {
    const expectedIds = ['new-project', 'open-project', 'open-model', 'save', 'save-as']
    expect(Object.keys(menuActionDispatch).sort()).toEqual(expectedIds.sort())
  })
})
