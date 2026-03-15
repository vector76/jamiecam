/**
 * Tests for PencilMillingEditor.tsx — renders fields for Pencil Milling
 * and calls onSave with partial params on blur/interaction.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import PencilMillingEditor from './PencilMillingEditor'
import { useViewportStore } from '../../store/viewportStore'
import type { PencilMillingParams } from '../../api/types'

// -- Module mocks -------------------------------------------------------------

vi.mock('../../api/geometry', () => ({
  getModelFaces: vi.fn(),
}))

const geoApi = await import('../../api/geometry')

// -- Fixtures -----------------------------------------------------------------

const BASE_PARAMS: PencilMillingParams = {
  allowance: 0,
  toolDiameter: 6.0,
  curvatureThreshold: null,
  minPassLength: 1.0,
  geometry: null,
  arcLeadInRadius: null,
  arcLeadOutRadius: null,
  helicalEntryRadius: null,
  helicalEntryPitch: null,
  rampEntryAngleDeg: null,
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

describe('PencilMillingEditor — rendering', () => {
  it('renders all required fields with correct labels', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Curvature threshold (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Min pass length (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Allowance (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Tool diameter (mm)')).toBeInTheDocument()
  })

  it('renders all five entry motion fields', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry radius (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toBeInTheDocument()
    expect(screen.getByLabelText('Ramp entry angle (°)')).toBeInTheDocument()
  })

  it('shows correct default values for required fields', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Min pass length (mm)')).toHaveValue(1.0)
    expect(screen.getByLabelText('Allowance (mm)')).toHaveValue(0)
    expect(screen.getByLabelText('Tool diameter (mm)')).toHaveValue(6.0)
  })

  it('curvature threshold shows empty (placeholder Auto) when null', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Curvature threshold (mm)')
    expect(input).toHaveValue(null)
    expect(input).toHaveAttribute('placeholder', 'Auto')
  })

  it('curvature threshold shows value when set', () => {
    const params: PencilMillingParams = { ...BASE_PARAMS, curvatureThreshold: 2.5 }
    render(<PencilMillingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Curvature threshold (mm)')).toHaveValue(2.5)
  })

  it('optional entry motion fields are empty when params omit them', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByLabelText('Arc lead-in radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Arc lead-out radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry radius (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Helical entry pitch (mm)')).toHaveValue(null)
    expect(screen.getByLabelText('Ramp entry angle (°)')).toHaveValue(null)
  })

  it('renders Select Faces button initially', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Select Faces')).toBeInTheDocument()
  })

  it('shows stock boundary default text when no geometry is set', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.getByText('Stock boundary (default)')).toBeInTheDocument()
  })

  it('shows face count when geometry is set', () => {
    const params: PencilMillingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<PencilMillingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('2 face(s) selected')).toBeInTheDocument()
  })

  it('Clear button appears when geometry is set', () => {
    const params: PencilMillingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<PencilMillingEditor params={params} onSave={vi.fn()} />)

    expect(screen.getByText('Clear')).toBeInTheDocument()
  })

  it('Clear button does not appear when geometry is absent', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    expect(screen.queryByText('Clear')).not.toBeInTheDocument()
  })
})

// -- onSave calls — required fields -------------------------------------------

describe('PencilMillingEditor — required field blur saves', () => {
  it('curvature threshold blur with value calls onSave with parsed curvatureThreshold', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Curvature threshold (mm)'), { target: { value: '2.5' } })

    expect(onSave).toHaveBeenCalledWith({ curvatureThreshold: 2.5 })
  })

  it('curvature threshold blur with blank calls onSave with null (Auto)', () => {
    const onSave = vi.fn()
    const params: PencilMillingParams = { ...BASE_PARAMS, curvatureThreshold: 2.5 }
    render(<PencilMillingEditor params={params} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Curvature threshold (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ curvatureThreshold: null })
  })

  it('min pass length blur calls onSave with parsed minPassLength', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Min pass length (mm)'), { target: { value: '2.0' } })

    expect(onSave).toHaveBeenCalledWith({ minPassLength: 2.0 })
  })

  it('allowance blur calls onSave with parsed allowance', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Allowance (mm)'), { target: { value: '0.05' } })

    expect(onSave).toHaveBeenCalledWith({ allowance: 0.05 })
  })

  it('tool diameter blur calls onSave with parsed toolDiameter', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Tool diameter (mm)'), { target: { value: '8.0' } })

    expect(onSave).toHaveBeenCalledWith({ toolDiameter: 8.0 })
  })
})

// -- onSave calls — entry motion fields ---------------------------------------

describe('PencilMillingEditor — entry motion field blur saves', () => {
  it('arc lead-in radius blur with value calls onSave with arcLeadInRadius', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '2.5' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: 2.5 })
  })

  it('arc lead-in radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-in radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadInRadius: null })
  })

  it('arc lead-out radius blur with value calls onSave with arcLeadOutRadius', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: 3 })
  })

  it('arc lead-out radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Arc lead-out radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ arcLeadOutRadius: null })
  })

  it('helical entry radius blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry radius (mm)'), { target: { value: '1.5' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryRadius: 1.5 })
  })

  it('helical entry radius blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry radius (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryRadius: null })
  })

  it('helical entry pitch blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry pitch (mm)'), { target: { value: '2' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryPitch: 2 })
  })

  it('helical entry pitch blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Helical entry pitch (mm)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ helicalEntryPitch: null })
  })

  it('ramp entry angle blur with value calls onSave', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Ramp entry angle (°)'), { target: { value: '3' } })

    expect(onSave).toHaveBeenCalledWith({ rampEntryAngleDeg: 3 })
  })

  it('ramp entry angle blur with blank calls onSave with null', () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.blur(screen.getByLabelText('Ramp entry angle (°)'), { target: { value: '' } })

    expect(onSave).toHaveBeenCalledWith({ rampEntryAngleDeg: null })
  })
})

// -- Face selection -----------------------------------------------------------

describe('PencilMillingEditor — face selection', () => {
  it('clicking Select Faces calls getModelFaces and sets selectionMode true', async () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(geoApi.getModelFaces).toHaveBeenCalled())
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))
  })

  it('while selectionMode is true, button text is Done Selecting', async () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))

    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())
    expect(screen.queryByText('Select Faces')).not.toBeInTheDocument()
  })

  it('clicking Done Selecting sets selectionMode false and calls onSave with fingerprints', async () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: ['fp-x', 'fp-y'] })
    fireEvent.click(screen.getByText('Done Selecting'))

    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(false))
    expect(onSave).toHaveBeenCalledWith({ geometry: ['fp-x', 'fp-y'] })
  })

  it('clicking Done Selecting with no selected faces calls onSave with null', async () => {
    const onSave = vi.fn()
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={onSave} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(screen.getByText('Done Selecting')).toBeInTheDocument())

    useViewportStore.setState({ selectedFaceFingerprints: [] })
    fireEvent.click(screen.getByText('Done Selecting'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('clicking Select Faces pre-populates fingerprints from saved geometry', async () => {
    const params: PencilMillingParams = { ...BASE_PARAMS, geometry: ['fp-a', 'fp-b'] }
    render(<PencilMillingEditor params={params} onSave={vi.fn()} />)

    fireEvent.click(screen.getByText('Select Faces'))
    await waitFor(() => expect(useViewportStore.getState().selectionMode).toBe(true))

    expect(useViewportStore.getState().selectedFaceFingerprints).toEqual(['fp-a', 'fp-b'])
  })

  it('clicking Clear calls onSave with geometry null', () => {
    const onSave = vi.fn()
    const params: PencilMillingParams = { ...BASE_PARAMS, geometry: ['fp-a'] }
    render(<PencilMillingEditor params={params} onSave={onSave} />)

    fireEvent.click(screen.getByText('Clear'))

    expect(onSave).toHaveBeenCalledWith({ geometry: null })
  })

  it('selectionMode true shows selected face count from viewport store', async () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    useViewportStore.setState({ selectionMode: true, selectedFaceFingerprints: ['fp-1', 'fp-2', 'fp-3'] })

    await waitFor(() => expect(screen.getByText('3 face(s) selected')).toBeInTheDocument())
  })
})

// -- Validation ---------------------------------------------------------------

describe('PencilMillingEditor — validation', () => {
  it('curvature threshold input has min attribute set to 0', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Curvature threshold (mm)')
    expect(input).toHaveAttribute('min', '0')
  })

  it('min pass length input has min attribute set to 0', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Min pass length (mm)')
    expect(input).toHaveAttribute('min', '0')
  })

  it('tool diameter input has min attribute set to positive value', () => {
    render(<PencilMillingEditor params={BASE_PARAMS} onSave={vi.fn()} />)

    const input = screen.getByLabelText('Tool diameter (mm)')
    expect(input).toHaveAttribute('min', '0.01')
  })
})
