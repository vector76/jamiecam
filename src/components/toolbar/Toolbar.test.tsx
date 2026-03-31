/**
 * Tests for Toolbar.tsx — verifies button rendering and delegation to
 * shared menu action handlers.
 *
 * Detailed handler logic is tested in src/lib/menuActions.test.ts.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { Toolbar } from './Toolbar'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../lib/menuActions', () => ({
  handleOpenModel: vi.fn().mockResolvedValue(undefined),
  handleNewProject: vi.fn().mockResolvedValue(undefined),
  handleSaveAs: vi.fn().mockResolvedValue(undefined),
  handleOpenProject: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../../api/window', () => ({
  openToolEditor: vi.fn().mockResolvedValue(undefined),
}))

const menuActions = await import('../../lib/menuActions')
const windowApi = await import('../../api/window')

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
})

// ── Button rendering ─────────────────────────────────────────────────────────

describe('Toolbar — button rendering', () => {
  it('renders Open Model button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /open model/i })).toBeInTheDocument()
  })

  it('renders New Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /new project/i })).toBeInTheDocument()
  })

  it('renders Save Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /save project/i })).toBeInTheDocument()
  })

  it('renders Open Project button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /open project/i })).toBeInTheDocument()
  })

  it('renders Tool Editor button', () => {
    render(<Toolbar />)
    expect(screen.getByRole('button', { name: /tool editor/i })).toBeInTheDocument()
  })
})

// ── Handler delegation ───────────────────────────────────────────────────────

describe('Toolbar — handler delegation', () => {
  it('calls handleOpenModel when Open Model is clicked', async () => {
    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open model/i }))

    await waitFor(() => expect(menuActions.handleOpenModel).toHaveBeenCalled())
  })

  it('calls handleNewProject when New Project is clicked', async () => {
    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /new project/i }))

    await waitFor(() => expect(menuActions.handleNewProject).toHaveBeenCalled())
  })

  it('calls handleSaveAs when Save Project is clicked', async () => {
    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /save project/i }))

    await waitFor(() => expect(menuActions.handleSaveAs).toHaveBeenCalled())
  })

  it('calls handleOpenProject when Open Project is clicked', async () => {
    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /open project/i }))

    await waitFor(() => expect(menuActions.handleOpenProject).toHaveBeenCalled())
  })

  it('calls openToolEditor when Tool Editor is clicked', async () => {
    render(<Toolbar />)
    fireEvent.click(screen.getByRole('button', { name: /tool editor/i }))

    await waitFor(() => expect(windowApi.openToolEditor).toHaveBeenCalled())
  })
})
