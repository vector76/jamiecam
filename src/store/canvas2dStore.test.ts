import { useCanvas2dStore } from './canvas2dStore'

beforeEach(() => {
  useCanvas2dStore.setState({
    panOffset: { x: 0, y: 0 },
    zoom: 1.0,
    selectedCurveId: null,
  })
})

describe('canvas2dStore — initial state', () => {
  it('starts with panOffset {x:0, y:0}', () => {
    expect(useCanvas2dStore.getState().panOffset).toEqual({ x: 0, y: 0 })
  })

  it('starts with zoom 1.0', () => {
    expect(useCanvas2dStore.getState().zoom).toBe(1.0)
  })

  it('starts with selectedCurveId null', () => {
    expect(useCanvas2dStore.getState().selectedCurveId).toBeNull()
  })
})

describe('canvas2dStore — setSelectedCurveId', () => {
  it('updates selectedCurveId to a string', () => {
    useCanvas2dStore.getState().setSelectedCurveId('curve-abc')
    expect(useCanvas2dStore.getState().selectedCurveId).toBe('curve-abc')
  })

  it('clears selectedCurveId when passed null', () => {
    useCanvas2dStore.getState().setSelectedCurveId('curve-abc')
    useCanvas2dStore.getState().setSelectedCurveId(null)
    expect(useCanvas2dStore.getState().selectedCurveId).toBeNull()
  })

  it('replaces a previously set ID', () => {
    useCanvas2dStore.getState().setSelectedCurveId('curve-1')
    useCanvas2dStore.getState().setSelectedCurveId('curve-2')
    expect(useCanvas2dStore.getState().selectedCurveId).toBe('curve-2')
  })
})

describe('canvas2dStore — setZoom clamping', () => {
  it('sets zoom within range', () => {
    useCanvas2dStore.getState().setZoom(2.5)
    expect(useCanvas2dStore.getState().zoom).toBe(2.5)
  })

  it('clamps zoom to minimum 0.05', () => {
    useCanvas2dStore.getState().setZoom(0.001)
    expect(useCanvas2dStore.getState().zoom).toBe(0.05)
  })

  it('clamps zoom to maximum 50.0', () => {
    useCanvas2dStore.getState().setZoom(999)
    expect(useCanvas2dStore.getState().zoom).toBe(50.0)
  })

  it('accepts exact minimum boundary value', () => {
    useCanvas2dStore.getState().setZoom(0.05)
    expect(useCanvas2dStore.getState().zoom).toBe(0.05)
  })

  it('accepts exact maximum boundary value', () => {
    useCanvas2dStore.getState().setZoom(50.0)
    expect(useCanvas2dStore.getState().zoom).toBe(50.0)
  })
})

describe('canvas2dStore — resetView', () => {
  it('restores default panOffset', () => {
    useCanvas2dStore.getState().setPanOffset({ x: 100, y: 200 })
    useCanvas2dStore.getState().resetView()
    expect(useCanvas2dStore.getState().panOffset).toEqual({ x: 0, y: 0 })
  })

  it('restores default zoom', () => {
    useCanvas2dStore.getState().setZoom(5.0)
    useCanvas2dStore.getState().resetView()
    expect(useCanvas2dStore.getState().zoom).toBe(1.0)
  })

  it('clears selectedCurveId', () => {
    useCanvas2dStore.getState().setSelectedCurveId('curve-xyz')
    useCanvas2dStore.getState().resetView()
    expect(useCanvas2dStore.getState().selectedCurveId).toBeNull()
  })
})
