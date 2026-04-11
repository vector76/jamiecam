/**
 * Tests for unsavedGuard.ts — the reusable guard that checks for unsaved
 * changes before destructive actions.
 */

import { useProjectStore } from '../store/projectStore'
import type { ProjectSnapshot } from '../api/types'

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(),
}))

vi.mock('../api/file', () => ({
  saveProject: vi.fn(),
  saveProjectCurrent: vi.fn(),
}))

const { save } = await import('@tauri-apps/plugin-dialog')
const { saveProject, saveProjectCurrent } = await import('../api/file')

// Lazy-import after mocks are in place.
const { checkUnsavedChanges } = await import('./unsavedGuard')

// ── Fixtures ─────────────────────────────────────────────────────────────────

const CLEAN_SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test',
  modifiedAt: '',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
  projectIsOpen: false,
  filePath: null,
  dirty: false,
  mode: '3d',
}

const DIRTY_SNAPSHOT: ProjectSnapshot = {
  ...CLEAN_SNAPSHOT,
  dirty: true,
}

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({
    snapshot: null,
    notifications: [],
    unsavedDialogOpen: false,
    unsavedDialogResolve: null,
  })
})

// ── Tests ────────────────────────────────────────────────────────────────────

describe('checkUnsavedChanges', () => {
  it('returns true immediately when not dirty', async () => {
    useProjectStore.setState({ snapshot: CLEAN_SNAPSHOT })
    const result = await checkUnsavedChanges()
    expect(result).toBe(true)
    expect(useProjectStore.getState().unsavedDialogOpen).toBe(false)
  })

  it('returns true when snapshot is null (not dirty)', async () => {
    const result = await checkUnsavedChanges()
    expect(result).toBe(true)
  })

  it('dirty + user chooses Save + filePath exists -> saveProjectCurrent called -> returns true', async () => {
    useProjectStore.setState({
      snapshot: { ...DIRTY_SNAPSHOT, filePath: '/tmp/project.jcam' },
    })

    const promise = checkUnsavedChanges()

    // Resolve the dialog as the user would.
    useProjectStore.getState().resolveUnsavedDialog('save')

    const result = await promise
    expect(saveProjectCurrent).toHaveBeenCalled()
    expect(result).toBe(true)
  })

  it('dirty + user chooses Save + no filePath -> save dialog opens -> saveProject called -> returns true', async () => {
    useProjectStore.setState({ snapshot: DIRTY_SNAPSHOT })
    vi.mocked(save).mockResolvedValue('/tmp/new.jcam')
    vi.mocked(saveProject).mockResolvedValue(undefined)

    const promise = checkUnsavedChanges()
    useProjectStore.getState().resolveUnsavedDialog('save')

    const result = await promise
    expect(save).toHaveBeenCalled()
    expect(saveProject).toHaveBeenCalledWith('/tmp/new.jcam')
    expect(result).toBe(true)
  })

  it('dirty + user chooses Save + save fails -> returns false, pushNotification called', async () => {
    useProjectStore.setState({
      snapshot: { ...DIRTY_SNAPSHOT, filePath: '/tmp/project.jcam' },
    })
    vi.mocked(saveProjectCurrent).mockRejectedValue(new Error('disk full'))

    const promise = checkUnsavedChanges()
    useProjectStore.getState().resolveUnsavedDialog('save')

    const result = await promise
    expect(result).toBe(false)
    expect(useProjectStore.getState().notifications).toContain('Failed to save project.')
  })

  it('dirty + user chooses Save + no filePath + user cancels save dialog -> returns false', async () => {
    useProjectStore.setState({ snapshot: DIRTY_SNAPSHOT })
    vi.mocked(save).mockResolvedValue(null)

    const promise = checkUnsavedChanges()
    useProjectStore.getState().resolveUnsavedDialog('save')

    const result = await promise
    expect(result).toBe(false)
    expect(saveProject).not.toHaveBeenCalled()
  })

  it("dirty + user chooses Don't Save -> returns true, no save called", async () => {
    useProjectStore.setState({ snapshot: DIRTY_SNAPSHOT })

    const promise = checkUnsavedChanges()
    useProjectStore.getState().resolveUnsavedDialog('discard')

    const result = await promise
    expect(result).toBe(true)
    expect(saveProjectCurrent).not.toHaveBeenCalled()
    expect(saveProject).not.toHaveBeenCalled()
  })

  it('dirty + user chooses Cancel -> returns false', async () => {
    useProjectStore.setState({ snapshot: DIRTY_SNAPSHOT })

    const promise = checkUnsavedChanges()
    useProjectStore.getState().resolveUnsavedDialog('cancel')

    const result = await promise
    expect(result).toBe(false)
  })
})
