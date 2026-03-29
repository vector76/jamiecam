/**
 * Tests for ParallelFinishingEditor.tsx — renders fields for Parallel Finishing
 * and calls onSave with partial params on blur/interaction.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import ParallelFinishingEditor from './ParallelFinishingEditor'
import { useViewportStore } from '../../store/viewportStore'
import type { ParallelFinishingParams } from '../../api/types'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/geometry', () => ({
  getModelFaces: vi.fn(),
}))

const geoApi = await import('../../api/geometry')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const BASE_PARAMS: ParallelFinishingParams = {
  stepover: 1.0,
  directionAngleDeg: 0,
  allowance: 0.1,
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useViewportStore.setState({
    selectionMode: false,
    selectedFaceFingerprints: [],
    faceDescriptors: [],
    hoveredFaceIdx: null,
  })
  vi.mocked(geoApi.getModelFaces).mockResolvedValue([])
})

// ── Field rendering ───────────────────────────────────────────────────────────

describe('ParallelFinishingEditor — rendering', () => {
  it('renders Stepover, Direction, and Allowance inputs', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Stepover (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Direction (deg)')).toBeInTheDocument()
    expect(screen.getByLabelText('Allowance (mm)')).toBeInTheDocument()
  })

  it('renders all five entry motion fields', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (deg)')).toBeInTheDocument()
  })

  it('shows correct default values for required fields', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Stepover (mm)')).toHaveValue(1.0)
    expect(screen.getByLabelText('Direction (deg)')).toHaveValue(0)
    expect(screen.getByLabelText('Allowance (mm)')).toHaveValue(0.1)
  })

  it('optional entry motion fields are empty when params omit them', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Ramp entry angle (deg)')).toHaveValue(null)
  })

  it('renders Select Faces button initially', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Select Faces')).toBeInTheDocument()
  })

  it('shows stock boundary default text when no geometry is set', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Stock boundary (default)')).toBeInTheDocument()
  })

  it('shows face count when geometry is set', () => {
    const params: ParallelFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<ParallelFinishingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('2 face(s) selected')).toBeInTheDocument()
  })

  it('Clear button appears when geometry is set', () => {
    const params: ParallelFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<ParallelFinishingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('Clear')).toBeInTheDocument()
  })

  it('Clear button does not appear when geometry is absent', () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.queryByText('Clear')).not.toBeInTheDocument()
  })
})

// ── onSave calls — required fields ────────────────────────────────────────────

describe('ParallelFinishingEditor — required field blur saves', () => {
  it('stepover blur calls onSave with parsed stepover', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Stepover (mm)'), { target: { value: '0.5' } })

    expect(onSave).toHaveBeenCalledWith({ stepover: 0.5 })
  })

  it('direction blur calls onSave with parsed directionAngleDeg', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Direction (deg)'), { target: { value: '45' } })

    expect(onSave).toHaveBeenCalledWith({ directionAngleDeg: 45 })
  })

  it('allowance blur calls onSave with parsed allowance', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Allowance (mm)'), { target: { value: '0.05' } })

    expect(onSave).toHaveBeenCalledWith({ allowance: 0.05 })
  })
})

// ── onSave calls — entry motion fields ────────────────────────────────────────

describe('ParallelFinishingEditor — entry motion field blur saves', () => {
  it('arc lead-in radius blur with value calls onSave with arcLeadInRadius', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '2.5' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: 2.5 })
  })

  it('arc lead-in radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: null })
  })

  it('arc lead-out radius blur with value calls onSave with arcLeadOutRadius', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: 3 })
  })

  it('arc lead-out radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: null })
  })

  it('helical entry radius blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry radius (mm)'), { target: { value: '1.5' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryRadius: 1.5 })
  })

  it('helical entry pitch blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry pitch (mm)'), { target: { value: '2' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryPitch: 2 })
  })

  it('ramp entry angle blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Ramp entry angle (deg)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ rampEntryAngleDeg: 3 })
  })
})

// ── Face selection ────────────────────────────────────────────────────────────

describe('ParallelFinishingEditor — face selection', () => {
  it('clicking Select Faces calls getModelFaces and sets selectionMode true', async () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(geoApi.getModelFaces).toHaveBeenCalled())
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('while selectionMode is true, button text is Done Selecting', async () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    expect(screen.queryByText('Select Faces')).not.toBeInTheDocument()
  })

  it('clicking Done Selecting sets selectionMode false and calls onSave with fingerprints', async () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: ['fp-x', 'fp-y'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(false))
    expect(onSave).toHaveBeenCalledWith({ geometry: ['fp-x', 'fp-y'] })
  })

  it('clicking Done Selecting with no selected faces calls onSave with null', async () => {
    const onSave = vi.fn()
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: [] })
    fireEvent.click(screen.getByText('Done Selecting'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('clicking Select Faces pre-populates fingerprints from saved geometry', async () => {
    const params: ParallelFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<ParallelFinishingEditor params={params} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))

    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-a', 'fp-b'])
  })

  it('clicking Clear calls onSave with geometry null', () => {
    const onSave = vi.fn()
    const params: ParallelFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<ParallelFinishingEditor params={params} onSave={onSave} />)

    fireEvent.click(screen.getByText('Clear'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('selectionMode true shows selected face count from viewport store', async () => {
    render(<ParallelFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    useViewportStore.setState({ selectionMode: true, selectedFaceFingerprints: ['fp-1', 'fp-2', 'fp-3'] })

    await waitFor(() => expect(screen.getByText('3 face(s) selected')).toBeInTheDocument())
  })
})
