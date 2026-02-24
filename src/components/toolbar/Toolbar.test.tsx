/**
 * Tests for Toolbar.tsx — file operation buttons and error notifications.
 *
 * @tauri-apps/plugin-dialog and the API layer are mocked so tests run in
 * jsdom without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { Toolbar } from './Toolbar'
import { useProjectStore } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import type { MeshData, ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
  save: vi.fn(),
}))

vi.mock('../../api/file', () => ({
  openModel: vi.fn(),
  newProject: vi.fn(),
  saveProject: vi.fn(),
  loadProject: vi.fn(),
  getProjectSnapshot: vi.fn(),
}))

// Dynamic import inside updateWindowTitle — mock the whole module.
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ setTitle: vi.fn() })),
}))

// Import mocked modules for control in tests.
const { open, save } = await import('@tauri-apps/plugin-dialog')
const api = await import('../../api/file')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MESH: MeshData = { vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0], normals: [0, 0, 1, 0, 0, 1, 0, 0, 1], indices: [0, 1, 2] }
const SNAPSHOT: ProjectSnapshot = { modelPath: '/models/part.step', modelChecksum: 'abc', projectName: 'Test', modifiedAt: '' }
const EMPTY_SNAPSHOT: ProjectSnapshot = { modelPath: null, modelChecksum: null, projectName: '', modifiedAt: '' }

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null })
  useViewportStore.setState({ meshData: null, orbitTarget: [0, 0, 0], zoom: 1 })
})

// ── Open Model ────────────────────────────────────────────────────────────────

describe('Toolbar — Open Model', () => {
  it('renders an Open Model button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /open model/i })).toBeInTheDocument()
  })

  it('calls openModel with the selected path', async () => {
    vi.mocked(open).mockResolvedValue('/models/part.step')
    vi.mocked(api.openModel).mockResolvedValue(MESH)
    vi.mocked(api.getProjectSnapshot).mockResolvedValue(SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(api.openModel).toHaveBeenCalledWith('/models/part.step'))
  })

  it('updates viewportStore.meshData on success', async () => {
    vi.mocked(open).mockResolvedValue('/models/part.step')
    vi.mocked(api.openModel).mockResolvedValue(MESH)
    vi.mocked(api.getProjectSnapshot).mockResolvedValue(SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(useViewportStore.getState().meshData).toEqual(MESH))
  })

  it('updates projectStore.snapshot on success', async () => {
    vi.mocked(open).mockResolvedValue('/models/part.step')
    vi.mocked(api.openModel).mockResolvedValue(MESH)
    vi.mocked(api.getProjectSnapshot).mockResolvedValue(SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT))
  })

  it('does nothing when the dialog is cancelled', async () => {
    vi.mocked(open).mockResolvedValue(null)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(api.openModel).not.toHaveBeenCalled())
  })

  it('shows an error notification when openModel throws', async () => {
    vi.mocked(open).mockResolvedValue('/bad.step')
    vi.mocked(api.openModel).mockRejectedValue({ kind: 'GeometryImport', message: 'Failed to parse file' })

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByText(/failed to parse file/i)).toBeInTheDocument()
  })

  it('error notification is dismissible', async () => {
    vi.mocked(open).mockResolvedValue('/bad.step')
    vi.mocked(api.openModel).mockRejectedValue({ kind: 'GeometryImport', message: 'Import error' })

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => screen.getByRole('alert'))
    fireEvent.click(screen.getByRole('button', { name: /dismiss error/i }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
  })
})

// ── New Project ───────────────────────────────────────────────────────────────

describe('Toolbar — New Project', () => {
  it('renders a New Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /new project/i })).toBeInTheDocument()
  })

  it('clears viewportStore.meshData', async () => {
    useViewportStore.setState({ meshData: MESH })
    vi.mocked(api.newProject).mockResolvedValue(EMPTY_SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /new project/i }))

    await waitFor(() => expect(useViewportStore.getState().meshData).toBeNull())
  })

  it('updates projectStore.snapshot', async () => {
    vi.mocked(api.newProject).mockResolvedValue(EMPTY_SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /new project/i }))

    await waitFor(() => expect(useProjectStore.getState().snapshot).toEqual(EMPTY_SNAPSHOT))
  })

  it('shows an error notification when newProject throws', async () => {
    vi.mocked(api.newProject).mockRejectedValue({ kind: 'Io', message: 'Disk full' })

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /new project/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
    expect(screen.getByText(/disk full/i)).toBeInTheDocument()
  })
})

// ── Save Project ──────────────────────────────────────────────────────────────

describe('Toolbar — Save Project', () => {
  it('renders a Save Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /save project/i })).toBeInTheDocument()
  })

  it('calls saveProject with the chosen path', async () => {
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockResolvedValue(undefined)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /save project/i }))

    await waitFor(() => expect(api.saveProject).toHaveBeenCalledWith('/output/project.jcam'))
  })

  it('does nothing when the save dialog is cancelled', async () => {
    vi.mocked(save).mockResolvedValue(null)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /save project/i }))

    await waitFor(() => expect(api.saveProject).not.toHaveBeenCalled())
  })

  it('shows an error notification when saveProject throws', async () => {
    vi.mocked(save).mockResolvedValue('/output/project.jcam')
    vi.mocked(api.saveProject).mockRejectedValue({ kind: 'Io', message: 'Permission denied' })

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /save project/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})

// ── Open Project ──────────────────────────────────────────────────────────────

describe('Toolbar — Open Project', () => {
  it('renders an Open Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /open project/i })).toBeInTheDocument()
  })

  it('calls loadProject with the chosen path and updates snapshot', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(EMPTY_SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open project/i }))

    await waitFor(() => expect(api.loadProject).toHaveBeenCalledWith('/projects/job.jcam'))
    expect(useProjectStore.getState().snapshot).toEqual(EMPTY_SNAPSHOT)
  })

  it('reloads the model mesh when snapshot has a modelPath', async () => {
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(SNAPSHOT) // SNAPSHOT has modelPath
    vi.mocked(api.openModel).mockResolvedValue(MESH)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open project/i }))

    await waitFor(() => expect(api.openModel).toHaveBeenCalledWith(SNAPSHOT.modelPath))
    expect(useViewportStore.getState().meshData).toEqual(MESH)
  })

  it('clears meshData when snapshot has no modelPath', async () => {
    useViewportStore.setState({ meshData: MESH })
    vi.mocked(open).mockResolvedValue('/projects/job.jcam')
    vi.mocked(api.loadProject).mockResolvedValue(EMPTY_SNAPSHOT)

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open project/i }))

    await waitFor(() => expect(useViewportStore.getState().meshData).toBeNull())
  })

  it('shows an error notification when loadProject throws', async () => {
    vi.mocked(open).mockResolvedValue('/bad.jcam')
    vi.mocked(api.loadProject).mockRejectedValue({ kind: 'FileNotFound' })

    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open project/i }))

    await waitFor(() => expect(screen.getByRole('alert')).toBeInTheDocument())
  })
})
