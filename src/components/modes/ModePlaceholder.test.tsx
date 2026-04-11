/**
 * Tests for ModePlaceholder.tsx — placeholder screen shown when a mode is
 * active but not yet implemented.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ModePlaceholder } from './ModePlaceholder'
import { MODES } from './modeConfig'
import { useProjectStore } from '../../store/projectStore'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../lib/unsavedGuard', () => ({
  checkUnsavedChanges: vi.fn(),
}))

const unsavedGuard = await import('../../lib/unsavedGuard')

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
})

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('ModePlaceholder — rendering', () => {
  it.each(MODES)('renders mode number and label for $id', ({ id, number, label }) => {
    render(<ModePlaceholder mode={id} />)
    expect(screen.getByText((t) => t.includes(`Mode ${number}`))).toBeInTheDocument()
    expect(screen.getByText((t) => t.includes(label))).toBeInTheDocument()
  })

  it('renders "Not yet implemented" message', () => {
    render(<ModePlaceholder mode={MODES[0].id} />)
    expect(screen.getByText(/not yet implemented/i)).toBeInTheDocument()
  })

  it('renders a "Back" button', () => {
    render(<ModePlaceholder mode={MODES[0].id} />)
    expect(screen.getByRole('button', { name: /back/i })).toBeInTheDocument()
  })
})

// ── Back button — clean project ───────────────────────────────────────────────

describe('ModePlaceholder — Back button (clean project)', () => {
  it('calls returnToSelector when checkUnsavedChanges returns true', async () => {
    vi.mocked(unsavedGuard.checkUnsavedChanges).mockResolvedValue(true)
    const returnToSelector = vi.fn()
    useProjectStore.setState({ returnToSelector } as never)

    render(<ModePlaceholder mode={MODES[0].id} />)
    fireEvent.click(screen.getByRole('button', { name: /back/i }))

    await waitFor(() => {
      expect(returnToSelector).toHaveBeenCalledOnce()
    })
  })
})

// ── Back button — dirty project, user cancels ─────────────────────────────────

describe('ModePlaceholder — Back button (user cancels)', () => {
  it('does not call returnToSelector when checkUnsavedChanges returns false', async () => {
    vi.mocked(unsavedGuard.checkUnsavedChanges).mockResolvedValue(false)
    const returnToSelector = vi.fn()
    useProjectStore.setState({ returnToSelector } as never)

    render(<ModePlaceholder mode={MODES[0].id} />)
    fireEvent.click(screen.getByRole('button', { name: /back/i }))

    await waitFor(() => {
      expect(unsavedGuard.checkUnsavedChanges).toHaveBeenCalledOnce()
    })
    expect(returnToSelector).not.toHaveBeenCalled()
  })
})
