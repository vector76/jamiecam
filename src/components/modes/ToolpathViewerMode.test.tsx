/**
 * Tests for ToolpathViewerMode — the only mode in the web build.
 *
 * The wasm-backed API is mocked so these tests don't need to load the
 * actual WebAssembly module; we just check that the component wires its
 * pieces together correctly: file picker invokes the API, sample fetch
 * uses BASE_URL, results populate the sidebar and the viewport store.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ToolpathViewerMode } from './ToolpathViewerMode'
import { useViewportStore } from '../../store/viewportStore'
import type { GcodeViewerLoadResult } from '../../api/types'

// ── Mocks ─────────────────────────────────────────────────────────────────────

vi.mock('../../api/gcodeViewer', () => ({
  loadGcodeForViewer: vi.fn(),
  simulateGcodeViewer: vi.fn(),
}))

vi.mock('../../viewport/Viewport', () => ({
  Viewport: () => <div data-testid="viewport-mock" />,
}))

import { loadGcodeForViewer, simulateGcodeViewer } from '../../api/gcodeViewer'

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SAMPLE_RESULT: GcodeViewerLoadResult = {
  stock: {
    stockType: 'box',
    width: 100,
    depth: 50,
    height: 10,
    origin: { x: 0, y: 0, z: 0 },
  },
  tools: [
    {
      number: 1,
      toolType: 'flat_endmill',
      diameter: 6,
      flutes: 4,
      material: 'carbide',
    },
  ],
  lineGeometry: {
    positions: [0, 0, 0, 1, 1, 1],
    colours: [1, 1, 1, 1, 1, 1],
    types: [1],
  },
  warnings: [],
}

beforeEach(() => {
  vi.clearAllMocks()
  useViewportStore.setState({ toolpathGeometry: null, simulationMeshData: null })
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ToolpathViewerMode', () => {
  it('renders Open File and Load Sample buttons', () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByText('Open File…')).toBeInTheDocument()
    expect(screen.getByText('Load Sample')).toBeInTheDocument()
  })

  it('loads a file via the hidden input and shows metadata', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)
    render(<ToolpathViewerMode />)

    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    const file = new File(['; @TOOL\nG0 X0\n'], 'test.nc', { type: 'text/plain' })

    fireEvent.change(input, { target: { files: [file] } })

    await waitFor(() => {
      expect(loadGcodeForViewer).toHaveBeenCalledWith('; @TOOL\nG0 X0\n')
    })

    await waitFor(() => {
      expect(screen.getByText('test.nc')).toBeInTheDocument()
      expect(screen.getByText('Stock')).toBeInTheDocument()
      expect(screen.getByText('Tool')).toBeInTheDocument()
    })

    expect(useViewportStore.getState().toolpathGeometry).toEqual(SAMPLE_RESULT.lineGeometry)
  })

  it('fetches the bundled sample when Load Sample is clicked', async () => {
    const fetchSpy = vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response('; @TOOL\nG0 X0\n', { status: 200 }),
    )
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)

    render(<ToolpathViewerMode />)
    fireEvent.click(screen.getByText('Load Sample'))

    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalledWith(expect.stringContaining('samples/demo-pocket.nc'))
      expect(loadGcodeForViewer).toHaveBeenCalledWith('; @TOOL\nG0 X0\n')
    })
  })

  it('surfaces load errors in the sidebar', async () => {
    vi.mocked(loadGcodeForViewer).mockRejectedValueOnce({
      kind: 'InvalidInput',
      message: 'bad gcode',
    })

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    const file = new File(['bad'], 'bad.nc', { type: 'text/plain' })
    fireEvent.change(input, { target: { files: [file] } })

    expect(await screen.findByRole('alert')).toHaveTextContent('bad gcode')
  })

  it('lists parser warnings', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce({
      ...SAMPLE_RESULT,
      warnings: [{ line: 5, message: 'Unknown G-code' }],
    })

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['x'], 'x.nc', { type: 'text/plain' })] },
    })

    expect(await screen.findByText(/Unknown G-code/)).toBeInTheDocument()
  })

  it('runs a simulation and sets the mesh on the viewport store', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)
    const fakeMesh = { vertices: [0, 0, 0], normals: [0, 0, 1], indices: [], faceGroups: [] }
    vi.mocked(simulateGcodeViewer).mockResolvedValueOnce(fakeMesh)

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'p.nc', { type: 'text/plain' })] },
    })

    const simulateBtn = await screen.findByRole('button', { name: 'Simulate' })
    fireEvent.click(simulateBtn)

    await waitFor(() => {
      expect(simulateGcodeViewer).toHaveBeenCalledWith(
        'G0 X0\n',
        expect.objectContaining({
          stock: expect.objectContaining({ width: 100, depth: 50, height: 10 }),
          toolDiameter: 6,
          resolution: 0.5,
        }),
      )
    })
    await waitFor(() => {
      expect(useViewportStore.getState().simulationMeshData).toBe(fakeMesh)
    })
  })

  it('surfaces simulation errors in the sidebar', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)
    vi.mocked(simulateGcodeViewer).mockRejectedValueOnce({
      kind: 'InvalidInput',
      message: 'tool diameter must be positive',
    })

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'p.nc', { type: 'text/plain' })] },
    })

    const simulateBtn = await screen.findByRole('button', { name: 'Simulate' })
    fireEvent.click(simulateBtn)

    expect(await screen.findByRole('alert')).toHaveTextContent('tool diameter must be positive')
  })

  it('clears stale stock dims when a new file has no @STOCK header', async () => {
    // Load file A with metadata, then file B without — the form must not
    // silently carry A's stock dims into B's simulation.
    const bareResult: GcodeViewerLoadResult = {
      stock: null,
      tools: [],
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings: [],
    }
    vi.mocked(loadGcodeForViewer)
      .mockResolvedValueOnce(SAMPLE_RESULT)
      .mockResolvedValueOnce(bareResult)

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement

    fireEvent.change(input, {
      target: { files: [new File(['x'], 'a.nc', { type: 'text/plain' })] },
    })
    // Width input gets populated from SAMPLE_RESULT's @STOCK width=100.
    await waitFor(() => {
      expect((screen.getByLabelText('Width') as HTMLInputElement).value).toBe('100')
    })

    fireEvent.change(input, {
      target: { files: [new File(['x'], 'b.nc', { type: 'text/plain' })] },
    })
    await waitFor(() => {
      expect((screen.getByLabelText('Width') as HTMLInputElement).value).toBe('')
      expect((screen.getByLabelText('Tool Ø') as HTMLInputElement).value).toBe('')
    })
  })

  it('blocks simulation when required inputs are missing', async () => {
    // Load a file that has no @STOCK or @TOOL metadata so the form is empty.
    const bareResult: GcodeViewerLoadResult = {
      stock: null,
      tools: [],
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings: [],
    }
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(bareResult)

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'bare.nc', { type: 'text/plain' })] },
    })

    const simulateBtn = await screen.findByRole('button', { name: 'Simulate' })
    fireEvent.click(simulateBtn)

    expect(await screen.findByRole('alert')).toHaveTextContent(/positive/)
    expect(simulateGcodeViewer).not.toHaveBeenCalled()
  })
})
