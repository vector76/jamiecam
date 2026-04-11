/**
 * Tests for bootstrapApp() in main.tsx.
 *
 * Verifies that the function registers the correct backend listeners and
 * does not call getProjectSnapshot.
 */

// ── Hoisted variables (available inside vi.mock factories) ────────────────────

const { mockOnCloseRequested } = vi.hoisted(() => ({
  mockOnCloseRequested: vi.fn().mockResolvedValue(undefined),
}))

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}))

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ onCloseRequested: mockOnCloseRequested })),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}))

vi.mock('./api/file', () => ({
  getProjectSnapshot: vi.fn(),
  openModel: vi.fn(),
  loadProject: vi.fn(),
  newProject: vi.fn(),
  saveProject: vi.fn(),
  saveProjectCurrent: vi.fn(),
}))

vi.mock('./api/globalTools', () => ({
  listGlobalTools: vi.fn().mockResolvedValue([]),
}))

vi.mock('./lib/menuActions', () => ({
  menuActionDispatch: {},
}))

vi.mock('./lib/unsavedGuard', () => ({
  checkUnsavedChanges: vi.fn(),
}))

// Prevent Tailwind CSS transform (slow in test environment)
vi.mock('./index.css', () => ({}))

// Prevent loading the full component tree
vi.mock('./App', () => ({ default: () => null }))
vi.mock('./components/tools/ToolEditorWindow', () => ({ ToolEditorWindow: () => null }))

vi.mock('react-dom/client', () => ({
  default: { createRoot: vi.fn(() => ({ render: vi.fn() })) },
  createRoot: vi.fn(() => ({ render: vi.fn() })),
}))

// ── Lazy imports ─────────────────────────────────────────────────────────────

import { useProjectStore } from './store/projectStore'
const { listen } = await import('@tauri-apps/api/event')
const { getCurrentWindow } = await import('@tauri-apps/api/window')
const { getProjectSnapshot } = await import('./api/file')
const { listGlobalTools } = await import('./api/globalTools')
const { checkUnsavedChanges } = await import('./lib/unsavedGuard')
const { bootstrapApp, bootstrapToolEditor } = await import('./main')

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null })
})

afterEach(() => {
  vi.unstubAllEnvs()
})

// ── Tests ────────────────────────────────────────────────────────────────────

describe('bootstrapApp', () => {
  it('registers project:modified and menu:action listeners and close guard when not using mock API', async () => {
    vi.stubEnv('VITE_MOCK_API', 'false')

    await bootstrapApp()

    expect(listen).toHaveBeenCalledWith('project:modified', expect.any(Function))
    expect(listen).toHaveBeenCalledWith('menu:action', expect.any(Function))
    expect(getCurrentWindow).toHaveBeenCalled()
    expect(mockOnCloseRequested).toHaveBeenCalledWith(expect.any(Function))
    expect(getProjectSnapshot).not.toHaveBeenCalled()
  })
})

describe('Tool editor path regression', () => {
  it('bootstrapToolEditor fetches project snapshot and global tools then registers listener', async () => {
    ;(getProjectSnapshot as ReturnType<typeof vi.fn>).mockResolvedValue({
      modelPath: null,
      modelChecksum: null,
      projectName: '',
      modifiedAt: '',
      tools: [],
      stock: null,
      wcs: [],
      operations: [],
      projectIsOpen: false,
      filePath: null,
      dirty: false,
      mode: '3d' as const,
    })

    await bootstrapToolEditor()

    // Both data sources are fetched (distinguishes this path from bootstrapApp,
    // which calls neither)
    expect(getProjectSnapshot).toHaveBeenCalled()
    expect(listGlobalTools).toHaveBeenCalled()

    // The project:modified listener is registered for live updates
    expect(listen).toHaveBeenCalledWith('project:modified', expect.any(Function))
  })
})

describe('Window close guard', () => {
  it('does not call preventDefault when snapshot is null', async () => {
    vi.stubEnv('VITE_MOCK_API', 'false')
    useProjectStore.setState({ snapshot: null })
    ;(checkUnsavedChanges as ReturnType<typeof vi.fn>).mockResolvedValue(true)

    await bootstrapApp()

    const handler = mockOnCloseRequested.mock.calls[0][0]
    const mockEvent = { preventDefault: vi.fn() }
    await handler(mockEvent)

    expect(mockEvent.preventDefault).not.toHaveBeenCalled()
  })

  it('calls preventDefault when user cancels on a dirty project', async () => {
    vi.stubEnv('VITE_MOCK_API', 'false')
    useProjectStore.setState({
      snapshot: {
        modelPath: null,
        modelChecksum: null,
        projectName: '',
        modifiedAt: '',
        tools: [],
        stock: null,
        wcs: [],
        operations: [],
        projectIsOpen: true,
        filePath: null,
        dirty: true,
        mode: '3d' as const,
        safeHeight: null,
        artworkOrigin: [0, 0] as [number, number],
      },
    })
    ;(checkUnsavedChanges as ReturnType<typeof vi.fn>).mockResolvedValue(false)

    await bootstrapApp()

    const handler = mockOnCloseRequested.mock.calls[0][0]
    const mockEvent = { preventDefault: vi.fn() }
    await handler(mockEvent)

    expect(mockEvent.preventDefault).toHaveBeenCalled()
  })
})
