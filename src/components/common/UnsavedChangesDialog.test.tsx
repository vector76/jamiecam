/**
 * Tests for UnsavedChangesDialog.tsx — modal confirmation for unsaved changes.
 */

import { render, screen, fireEvent } from '@testing-library/react'
import { UnsavedChangesDialog } from './UnsavedChangesDialog'
import { useProjectStore } from '../../store/projectStore'

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({
    unsavedDialogOpen: false,
    unsavedDialogResolve: null,
  })
})

// ── Tests ────────────────────────────────────────────────────────────────────

describe('UnsavedChangesDialog', () => {
  it('renders nothing when unsavedDialogOpen is false', () => {
    render(<UnsavedChangesDialog />)
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
  })

  it('renders dialog with three buttons when open', () => {
    useProjectStore.setState({
      unsavedDialogOpen: true,
      unsavedDialogResolve: vi.fn(),
    })
    render(<UnsavedChangesDialog />)

    expect(screen.getByRole('dialog')).toBeInTheDocument()
    expect(screen.getByText('You have unsaved changes. Save before continuing?')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Save' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: "Don't Save" })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument()
  })

  it('clicking Save calls resolveUnsavedDialog with "save"', () => {
    const resolveFn = vi.fn()
    useProjectStore.setState({
      unsavedDialogOpen: true,
      unsavedDialogResolve: resolveFn,
    })
    render(<UnsavedChangesDialog />)

    fireEvent.click(screen.getByRole('button', { name: 'Save' }))

    expect(resolveFn).toHaveBeenCalledWith('save')
    expect(useProjectStore.getState().unsavedDialogOpen).toBe(false)
  })

  it('clicking Don\'t Save calls resolveUnsavedDialog with "discard"', () => {
    const resolveFn = vi.fn()
    useProjectStore.setState({
      unsavedDialogOpen: true,
      unsavedDialogResolve: resolveFn,
    })
    render(<UnsavedChangesDialog />)

    fireEvent.click(screen.getByRole('button', { name: "Don't Save" }))

    expect(resolveFn).toHaveBeenCalledWith('discard')
    expect(useProjectStore.getState().unsavedDialogOpen).toBe(false)
  })

  it('clicking Cancel calls resolveUnsavedDialog with "cancel"', () => {
    const resolveFn = vi.fn()
    useProjectStore.setState({
      unsavedDialogOpen: true,
      unsavedDialogResolve: resolveFn,
    })
    render(<UnsavedChangesDialog />)

    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }))

    expect(resolveFn).toHaveBeenCalledWith('cancel')
    expect(useProjectStore.getState().unsavedDialogOpen).toBe(false)
  })
})
