/**
 * Tests for ModeSelector.tsx — landing screen for choosing a CNC operation mode.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import { ModeSelector } from './ModeSelector'
import { MODES } from './modeConfig'
import { useProjectStore } from '../../store/projectStore'
import type { ProjectSnapshot } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/file', () => ({
  newProject: vi.fn(),
}))

vi.mock('../../lib/menuActions', () => ({
  handleOpenProject: vi.fn(),
}))

const apiFile = await import('../../api/file')
const menuActions = await import('../../lib/menuActions')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MOCK_SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'New Project',
  modifiedAt: '',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
  projectIsOpen: true,
  filePath: null,
  dirty: false,
  mode: 'gcode_viewer',
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
})

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('ModeSelector — rendering', () => {
  it('renders exactly seven mode buttons', () => {
    render(<ModeSelector />)
    const modeButtons = MODES.map((mode) => screen.getByRole('button', { name: mode.label }))
    expect(modeButtons).toHaveLength(7)
  })

  it('renders each mode with the correct label and description', () => {
    render(<ModeSelector />)
    for (const mode of MODES) {
      expect(screen.getByRole('button', { name: mode.label })).toBeInTheDocument()
      expect(screen.getByText(mode.description)).toBeInTheDocument()
    }
  })

  it('renders an "Open Project" button', () => {
    render(<ModeSelector />)
    expect(screen.getByRole('button', { name: /open project/i })).toBeInTheDocument()
  })

  it('all buttons are enabled initially', () => {
    render(<ModeSelector />)
    for (const mode of MODES) {
      expect(screen.getByRole('button', { name: mode.label })).not.toBeDisabled()
    }
    expect(screen.getByRole('button', { name: /open project/i })).not.toBeDisabled()
  })
})

// ── Interactions ──────────────────────────────────────────────────────────────

describe('ModeSelector — clicking a mode', () => {
  it('calls newProject with the correct mode string', async () => {
    vi.mocked(apiFile.newProject).mockResolvedValue(MOCK_SNAPSHOT)

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(apiFile.newProject).toHaveBeenCalledWith(MODES[0].id)
    })
  })

  it('calls newProject with the correct mode for each mode button', async () => {
    vi.mocked(apiFile.newProject).mockResolvedValue(MOCK_SNAPSHOT)

    render(<ModeSelector />)

    for (const mode of MODES) {
      vi.clearAllMocks()
      vi.mocked(apiFile.newProject).mockResolvedValue({ ...MOCK_SNAPSHOT, mode: mode.id })

      fireEvent.click(screen.getByRole('button', { name: mode.label }))

      await waitFor(() => {
        expect(apiFile.newProject).toHaveBeenCalledWith(mode.id)
      })
    }
  })

  it('calls setSnapshot with the returned snapshot after newProject resolves', async () => {
    vi.mocked(apiFile.newProject).mockResolvedValue(MOCK_SNAPSHOT)

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(useProjectStore.getState().snapshot).toEqual(MOCK_SNAPSHOT)
    })
  })

  it('pushes the error message on failure', async () => {
    vi.mocked(apiFile.newProject).mockRejectedValue({ kind: 'InvalidInput', message: 'bad mode' })

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(useProjectStore.getState().notifications).toContain('bad mode')
    })
  })

  it('falls back to error kind when message is absent', async () => {
    vi.mocked(apiFile.newProject).mockRejectedValue({ kind: 'FileNotFound' })

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(useProjectStore.getState().notifications).toContain('FileNotFound')
    })
  })
})

// ── Loading state ─────────────────────────────────────────────────────────────

describe('ModeSelector — loading state', () => {
  it('disables all mode buttons while newProject is in flight', async () => {
    vi.mocked(apiFile.newProject).mockImplementation(() => new Promise(() => {}))

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(screen.getByRole('button', { name: MODES[0].label })).toBeDisabled()
    })

    for (const mode of MODES) {
      expect(screen.getByRole('button', { name: mode.label })).toBeDisabled()
    }
    expect(screen.getByRole('button', { name: /open project/i })).toBeDisabled()
  })

  it('re-enables buttons after newProject resolves', async () => {
    vi.mocked(apiFile.newProject).mockResolvedValue(MOCK_SNAPSHOT)

    render(<ModeSelector />)
    fireEvent.click(screen.getByRole('button', { name: MODES[0].label }))

    await waitFor(() => {
      expect(screen.getByRole('button', { name: MODES[0].label })).not.toBeDisabled()
    })
  })
})

// ── Open Project ──────────────────────────────────────────────────────────────

describe('ModeSelector — Open Project button', () => {
  it('calls handleOpenProject when clicked', async () => {
    render(<ModeSelector />)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /open project/i }))
    })
    expect(menuActions.handleOpenProject).toHaveBeenCalledOnce()
  })
})
