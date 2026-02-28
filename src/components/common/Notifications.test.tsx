/**
 * Tests for Notifications.tsx — toast overlay driven by the project store.
 */

import { act, render, screen, fireEvent, waitFor } from '@testing-library/react'
import { Notifications } from './Notifications'
import { useProjectStore } from '../../store/projectStore'

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, notifications: [] })
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('Notifications', () => {
  it('renders nothing when there are no notifications', () => {
    render(<Notifications />)
    expect(document.body.textContent).toBe('')
  })

  it('renders a toast when a notification is added', () => {
    useProjectStore.setState({ notifications: ['something went wrong'] })
    render(<Notifications />)
    expect(screen.getByText('something went wrong')).toBeInTheDocument()
  })

  it('renders multiple toasts when multiple notifications are present', () => {
    useProjectStore.setState({ notifications: ['error one', 'error two'] })
    render(<Notifications />)
    expect(screen.getByText('error one')).toBeInTheDocument()
    expect(screen.getByText('error two')).toBeInTheDocument()
  })

  it('clicking × dismisses the toast', async () => {
    useProjectStore.setState({ notifications: ['something went wrong'] })
    render(<Notifications />)

    const dismissBtn = screen.getByRole('button', { name: 'Dismiss notification' })
    fireEvent.click(dismissBtn)

    await waitFor(() =>
      expect(screen.queryByText('something went wrong')).not.toBeInTheDocument()
    )
  })

  it('auto-dismisses after 5 seconds', async () => {
    vi.useFakeTimers()
    useProjectStore.setState({ notifications: ['something went wrong'] })
    render(<Notifications />)

    expect(screen.getByText('something went wrong')).toBeInTheDocument()

    await act(async () => { vi.advanceTimersByTime(5000) })

    expect(screen.queryByText('something went wrong')).not.toBeInTheDocument()

    vi.useRealTimers()
  })
})
