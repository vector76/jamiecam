/**
 * Tests for ScallopFinishingEditor.tsx — renders fields for Scallop Finishing
 * and calls onSave with partial params on blur/interaction.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import ScallopFinishingEditor from './ScallopFinishingEditor'
import { useViewportStore } from '../../store/viewportStore'
import type { ScallopFinishingParams } from '../../api/types'

// -- Module mocks -------------------------------------------------------------

vi.mock('../../api/geometry', () => ({
  getModelFaces: vi.fn(),
}))

const geoApi = await import('../../api/geometry')

// -- Fixtures -----------------------------------------------------------------

const BASE_PARAMS: ScallopFinishingParams = {
  targetScallopHeight: 0.05,
  minStepover: 0.5,
  maxStepover: 2.0,
  directionAngleDeg: 0,
  allowance: 0.1,
  toolRadius: 3.0,
}

// -- Setup --------------------------------------------------------------------

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

// -- Field rendering ----------------------------------------------------------

describe('ScallopFinishingEditor — rendering', () => {
  it('renders all required fields with correct labels', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Target scallop height (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Min stepover (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Max stepover (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Direction (deg)')).toBeInTheDocument()
    expect(screen.getByLabelText('Allowance (mm)')).toBeInTheDocument()
  })

  it('renders all five entry motion fields', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (deg)')).toBeInTheDocument()
  })

  it('shows correct default values for required fields', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Target scallop height (mm)')).toHaveValue(0.05)
    expect(screen.getByLabelText('Min stepover (mm)')).toHaveValue(0.5)
    expect(screen.getByLabelText('Max stepover (mm)')).toHaveValue(2.0)
    expect(screen.getByLabelText('Direction (deg)')).toHaveValue(0)
    expect(screen.getByLabelText('Allowance (mm)')).toHaveValue(0.1)
  })

  it('optional entry motion fields are empty when params omit them', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Ramp entry angle (deg)')).toHaveValue(null)
  })

  it('renders Select Faces button initially', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Select Faces')).toBeInTheDocument()
  })

  it('shows stock boundary default text when no geometry is set', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Stock boundary (default)')).toBeInTheDocument()
  })

  it('shows face count when geometry is set', () => {
    const params: ScallopFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<ScallopFinishingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('2 face(s) selected')).toBeInTheDocument()
  })

  it('Clear button appears when geometry is set', () => {
    const params: ScallopFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<ScallopFinishingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('Clear')).toBeInTheDocument()
  })

  it('Clear button does not appear when geometry is absent', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.queryByText('Clear')).not.toBeInTheDocument()
  })
})

// -- onSave calls — required fields -------------------------------------------

describe('ScallopFinishingEditor — required field blur saves', () => {
  it('target scallop height blur calls onSave with parsed targetScallopHeight', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Target scallop height (mm)'), { target: { value: '0.08' } })

    expect(onSave).toHaveBeenCalledWith({ targetScallopHeight: 0.08 })
  })

  it('min stepover blur calls onSave with parsed minStepover', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Min stepover (mm)'), { target: { value: '0.3' } })

    expect(onSave).toHaveBeenCalledWith({ minStepover: 0.3 })
  })

  it('max stepover blur calls onSave with parsed maxStepover', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Max stepover (mm)'), { target: { value: '3.0' } })

    expect(onSave).toHaveBeenCalledWith({ maxStepover: 3.0 })
  })

  it('direction blur calls onSave with parsed directionAngleDeg', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Direction (deg)'), { target: { value: '45' } })

    expect(onSave).toHaveBeenCalledWith({ directionAngleDeg: 45 })
  })

  it('allowance blur calls onSave with parsed allowance', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Allowance (mm)'), { target: { value: '0.05' } })

    expect(onSave).toHaveBeenCalledWith({ allowance: 0.05 })
  })
})

// -- onSave calls — entry motion fields ---------------------------------------

describe('ScallopFinishingEditor — entry motion field blur saves', () => {
  it('arc lead-in radius blur with value calls onSave with arcLeadInRadius', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '2.5' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: 2.5 })
  })

  it('arc lead-in radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: null })
  })

  it('arc lead-out radius blur with value calls onSave with arcLeadOutRadius', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: 3 })
  })

  it('arc lead-out radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: null })
  })

  it('helical entry radius blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry radius (mm)'), { target: { value: '1.5' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryRadius: 1.5 })
  })

  it('helical entry radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryRadius: null })
  })

  it('helical entry pitch blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry pitch (mm)'), { target: { value: '2' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryPitch: 2 })
  })

  it('helical entry pitch blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry pitch (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryPitch: null })
  })

  it('ramp entry angle blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Ramp entry angle (deg)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ rampEntryAngleDeg: 3 })
  })

  it('ramp entry angle blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Ramp entry angle (deg)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ rampEntryAngleDeg: null })
  })
})

// -- Face selection -----------------------------------------------------------

describe('ScallopFinishingEditor — face selection', () => {
  it('clicking Select Faces calls getModelFaces and sets selectionMode true', async () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(geoApi.getModelFaces).toHaveBeenCalled())
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('while selectionMode is true, button text is Done Selecting', async () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    expect(screen.queryByText('Select Faces')).not.toBeInTheDocument()
  })

  it('clicking Done Selecting sets selectionMode false and calls onSave with fingerprints', async () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: ['fp-x', 'fp-y'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(false))
    expect(onSave).toHaveBeenCalledWith({ geometry: ['fp-x', 'fp-y'] })
  })

  it('clicking Done Selecting with no selected faces calls onSave with null', async () => {
    const onSave = vi.fn()
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: [] })
    fireEvent.click(screen.getByText('Done Selecting'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('clicking Select Faces pre-populates fingerprints from saved geometry', async () => {
    const params: ScallopFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<ScallopFinishingEditor params={params} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))

    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-a', 'fp-b'])
  })

  it('clicking Clear calls onSave with geometry null', () => {
    const onSave = vi.fn()
    const params: ScallopFinishingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<ScallopFinishingEditor params={params} onSave={onSave} />)

    fireEvent.click(screen.getByText('Clear'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('selectionMode true shows selected face count from viewport store', async () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    useViewportStore.setState({ selectionMode: true, selectedFaceFingerprints: ['fp-1', 'fp-2', 'fp-3'] })

    await waitFor(() => expect(screen.getByText('3 face(s) selected')).toBeInTheDocument())
  })
})

// -- Validation ---------------------------------------------------------------

describe('ScallopFinishingEditor — validation', () => {
  it('scallop height input has min attribute set to positive value', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Target scallop height (mm)')
    expect(input).toHaveAttribute('min', '0.001')
  })

  it('min stepover input has min attribute set to positive value', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Min stepover (mm)')
    expect(input).toHaveAttribute('min', '0.01')
  })

  it('max stepover input has min attribute set to positive value', () => {
    render(<ScallopFinishingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Max stepover (mm)')
    expect(input).toHaveAttribute('min', '0.01')
  })
})
