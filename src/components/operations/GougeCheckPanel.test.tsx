/**
 * Tests for GougeCheckPanel.tsx — renders gouge-check UI, displays pass/fail
 * results, and triggers auto-lift with re-check.
 */

import { render, screen, fireEvent, waitFor, act } from '@testing-library/react'
import GougeCheckPanel from './GougeCheckPanel'
import type { GougeCheckResult } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/toolpath', () => ({
  checkGouge: vi.fn(),
  autoLift: vi.fn(),
}))

const toolpathApi = await import('../../api/toolpath')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const OP_ID = 'op-abc-123'

const PASS_RESULT: GougeCheckResult = { passed: true, violations: [] }

const FAIL_RESULT: GougeCheckResult = {
  passed: false,
  violations: [{ position: [1, 2, 3], gougeDepth: 0.5, faceIndex: 0 }],
}

const MULTI_VIOLATION_RESULT: GougeCheckResult = {
  passed: false,
  violations: [
    { position: [1, 2, 3], gougeDepth: 0.5, faceIndex: 0 },
    { position: [4.123, 5.456, 6.789], gougeDepth: 0.1234, faceIndex: 1 },
  ],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
})

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('GougeCheckPanel — rendering', () => {
  it('renders "Check Gouges" button on initial render', () => {
    render(<GougeCheckPanel operationId={OP_ID} />)

    expect(screen.getByText('Check Gouges')).toBeInTheDocument()
  })

  it('does not show pass/fail indicator initially', () => {
    render(<GougeCheckPanel operationId={OP_ID} />)

    expect(screen.queryByText('No gouges')).not.toBeInTheDocument()
    expect(screen.queryByText(/violation/)).not.toBeInTheDocument()
  })
})

// ── Check Gouges ──────────────────────────────────────────────────────────────

describe('GougeCheckPanel — check gouges', () => {
  it('clicking button triggers checkGouge with correct operationId', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(PASS_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))

    await waitFor(() => expect(toolpathApi.checkGouge).toHaveBeenCalledWith(OP_ID))
  })

  it('displays green indicator with "No gouges" on pass', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(PASS_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))

    await waitFor(() => expect(screen.getByText(/No gouges/)).toBeInTheDocument())
    expect(screen.getByText(/No gouges/).closest('span')).toHaveClass('text-success')
  })

  it('displays red indicator with violation count on fail', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(FAIL_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))

    await waitFor(() => expect(screen.getByText(/1 violation\b/)).toBeInTheDocument())
    expect(screen.getByText(/1 violation\b/).closest('span')).toHaveClass('text-destructive')
  })
})

// ── Violation list ────────────────────────────────────────────────────────────

describe('GougeCheckPanel — violation list', () => {
  it('renders XYZ coordinates and depth for each violation', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(MULTI_VIOLATION_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))
    await waitFor(() => expect(screen.getByText(/2 violations/)).toBeInTheDocument())

    // Coordinates are formatted with toFixed(3), depth with toFixed(4)
    expect(screen.getByText(/1\.000.*2\.000.*3\.000/)).toBeInTheDocument()
    expect(screen.getByText(/0\.5000/)).toBeInTheDocument()
    expect(screen.getByText(/4\.123.*5\.456.*6\.789/)).toBeInTheDocument()
    expect(screen.getByText(/0\.1234/)).toBeInTheDocument()
  })
})

// ── Auto-Lift button visibility ───────────────────────────────────────────────

describe('GougeCheckPanel — auto-lift visibility', () => {
  it('Auto-Lift button appears when violations > 0', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(FAIL_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))
    await waitFor(() => expect(screen.getByText('Auto-Lift')).toBeInTheDocument())
  })

  it('Auto-Lift button does not appear when passed', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(PASS_RESULT)
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))
    await waitFor(() => expect(screen.getByText(/No gouges/)).toBeInTheDocument())

    expect(screen.queryByText('Auto-Lift')).not.toBeInTheDocument()
  })
})

// ── Auto-Lift triggers API + re-check ─────────────────────────────────────────

describe('GougeCheckPanel — auto-lift action', () => {
  it('clicking Auto-Lift calls autoLift then re-runs checkGouge', async () => {
    vi.mocked(toolpathApi.checkGouge)
      .mockResolvedValueOnce(FAIL_RESULT) // initial check
      .mockResolvedValueOnce(PASS_RESULT) // re-check after auto-lift
    vi.mocked(toolpathApi.autoLift).mockResolvedValue(1)
    render(<GougeCheckPanel operationId={OP_ID} />)

    // First: run check to get violations
    fireEvent.click(screen.getByText('Check Gouges'))
    await waitFor(() => expect(screen.getByText('Auto-Lift')).toBeInTheDocument())

    // Then: click Auto-Lift
    fireEvent.click(screen.getByText('Auto-Lift'))

    await waitFor(() => expect(toolpathApi.autoLift).toHaveBeenCalledWith(OP_ID))
    await waitFor(() => expect(toolpathApi.checkGouge).toHaveBeenCalledTimes(2))
    // After auto-lift + re-check, should show pass
    await waitFor(() => expect(screen.getByText(/No gouges/)).toBeInTheDocument())
  })
})

// ── Loading state ─────────────────────────────────────────────────────────────

describe('GougeCheckPanel — loading state', () => {
  it('button is disabled and shows "Checking..." during API call', async () => {
    let resolve!: (value: GougeCheckResult) => void
    vi.mocked(toolpathApi.checkGouge).mockImplementation(
      () => new Promise((r) => { resolve = r })
    )
    render(<GougeCheckPanel operationId={OP_ID} />)

    fireEvent.click(screen.getByText('Check Gouges'))

    await waitFor(() => {
      const btn = screen.getByText('Checking...')
      expect(btn).toBeInTheDocument()
      expect(btn).toBeDisabled()
    })

    // Resolve the promise to clean up
    await act(async () => { resolve(PASS_RESULT) })
    await waitFor(() => expect(screen.getByText('Check Gouges')).toBeInTheDocument())
  })
})

// ── toolpathVersion change clears result ──────────────────────────────────────

describe('GougeCheckPanel — toolpathVersion change', () => {
  it('clears result when toolpathVersion prop changes', async () => {
    vi.mocked(toolpathApi.checkGouge).mockResolvedValue(PASS_RESULT)
    const { rerender } = render(<GougeCheckPanel operationId={OP_ID} toolpathVersion={1} />)

    fireEvent.click(screen.getByText('Check Gouges'))
    await waitFor(() => expect(screen.getByText(/No gouges/)).toBeInTheDocument())

    // Change toolpathVersion — result should clear
    rerender(<GougeCheckPanel operationId={OP_ID} toolpathVersion={2} />)

    await waitFor(() => {
      expect(screen.queryByText(/No gouges/)).not.toBeInTheDocument()
      expect(screen.queryByText(/violation/)).not.toBeInTheDocument()
    })
  })
})
