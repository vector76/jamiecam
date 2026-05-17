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
}))

vi.mock('../../viewport/Viewport', () => ({
  Viewport: () => <div data-testid="viewport-mock" />,
}))

import { loadGcodeForViewer } from '../../api/gcodeViewer'

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
})
