/**
 * Tests for ToolpathViewerMode — the only mode in the web build.
 *
 * The wasm-backed API is mocked so these tests don't need to load the
 * actual WebAssembly module; we just check that the component wires its
 * pieces together correctly: file picker invokes the API, sample fetch
 * uses BASE_URL, results populate the sidebar and the viewport store.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { IDBFactory } from 'fake-indexeddb'
import { ToolpathViewerMode } from './ToolpathViewerMode'
import { useViewportStore } from '../../store/viewportStore'
import type { GcodeViewerLoadResult } from '../../api/types'
import { __resetRecentsForTests, upsertRecent } from '../../persistence/recents'
import { packJcamProject, type ProjectState } from '../../persistence/projectFile'

// ── Mocks ─────────────────────────────────────────────────────────────────────

vi.mock('../../api/gcodeViewer', () => ({
  loadGcodeForViewer: vi.fn(),
  simulateGcodeViewer: vi.fn(),
  prewarmWasm: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../../viewport/Viewport', () => ({
  Viewport: () => <div data-testid="viewport-mock" />,
}))

import { loadGcodeForViewer, prewarmWasm, simulateGcodeViewer } from '../../api/gcodeViewer'

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
  // Fresh IndexedDB per test so recents written by one test don't leak
  // into the next (e.g. cause a "Recent" section to appear unexpectedly).
  globalThis.indexedDB = new IDBFactory()
  __resetRecentsForTests()
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ToolpathViewerMode', () => {
  it('renders the file-action buttons', async () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    expect(screen.getByText('Open Project…')).toBeInTheDocument()
    expect(screen.getByText('Save Project')).toBeInTheDocument()
    expect(screen.getByText('Load Sample')).toBeInTheDocument()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('shows an engine-initializing indicator until prewarm resolves', async () => {
    // Hold the prewarm promise open so we can observe the "Initializing…" state.
    let resolvePrewarm!: () => void
    vi.mocked(prewarmWasm).mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolvePrewarm = resolve
      }),
    )
    render(<ToolpathViewerMode />)
    expect(screen.getByText('Initializing engine…')).toBeInTheDocument()

    resolvePrewarm()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('surfaces an error if the wasm engine fails to initialize', async () => {
    vi.mocked(prewarmWasm).mockRejectedValueOnce({
      kind: 'Io',
      message: 'failed to fetch wasm',
    })
    render(<ToolpathViewerMode />)

    await waitFor(() => {
      expect(screen.getByText(/Engine failed to load: failed to fetch wasm/)).toBeInTheDocument()
    })
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

  it('Save Project is disabled until a G-code file is loaded', async () => {
    render(<ToolpathViewerMode />)
    expect(screen.getByText('Save Project')).toBeDisabled()
    // Wait for the prewarm useEffect to settle so its setState doesn't
    // fire after the test returns (would trigger an act() warning).
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('Save Project triggers a .jcam download after loading a file', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)

    const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:fake')
    const revokeObjectURL = vi.fn<(url: string) => void>()
    vi.stubGlobal('URL', { ...URL, createObjectURL, revokeObjectURL })
    const anchorClick = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {})

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'pocket.nc', { type: 'text/plain' })] },
    })
    await waitFor(() => expect(screen.getByText('Save Project')).not.toBeDisabled())

    fireEvent.click(screen.getByText('Save Project'))

    expect(createObjectURL).toHaveBeenCalledTimes(1)
    const blob = createObjectURL.mock.calls[0][0]
    expect(blob.type).toBe('application/zip')
    expect(blob.size).toBeGreaterThan(0)
    expect(anchorClick).toHaveBeenCalledTimes(1)
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:fake')

    anchorClick.mockRestore()
    vi.unstubAllGlobals()
  })

  it('Open Project restores the saved sim params (not the file metadata)', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce(SAMPLE_RESULT)

    // Saved sim uses different dims than SAMPLE_RESULT's @STOCK metadata
    // (200×60×8 vs 100×50×10) so we can tell which one wins on restore.
    const saved: ProjectState = {
      fileName: 'archived.nc',
      mode: 'gcode-viewer',
      payload: {
        gcode: '; some other gcode\nG0 X0\n',
        sim: {
          stock: { origin: { x: 1, y: 2, z: 3 }, width: 200, depth: 60, height: 8 },
          toolDiameter: 12,
          resolution: 0.25,
        },
      },
    }
    const bytes = packJcamProject(saved)
    const jcamFile = new File([new Uint8Array(bytes)], 'archived.jcam', {
      type: 'application/zip',
    })

    render(<ToolpathViewerMode />)
    const projectInput = screen.getByLabelText('Project file') as HTMLInputElement
    fireEvent.change(projectInput, { target: { files: [jcamFile] } })

    await waitFor(() => {
      expect((screen.getByLabelText('Width') as HTMLInputElement).value).toBe('200')
    })
    expect((screen.getByLabelText('Tool Ø') as HTMLInputElement).value).toBe('12')
    expect((screen.getByLabelText('Resolution') as HTMLInputElement).value).toBe('0.25')
    expect((screen.getByLabelText('Origin Z') as HTMLInputElement).value).toBe('3')
    if (saved.mode !== 'gcode-viewer') throw new Error('fixture mode mismatch')
    expect(loadGcodeForViewer).toHaveBeenCalledWith(saved.payload.gcode)
  })

  it('Open Project shows a clear error for a non-jcam file', async () => {
    render(<ToolpathViewerMode />)
    const projectInput = screen.getByLabelText('Project file') as HTMLInputElement
    fireEvent.change(projectInput, {
      target: { files: [new File(['hello'], 'junk.jcam', { type: 'application/zip' })] },
    })
    expect(await screen.findByRole('alert')).toHaveTextContent(/valid zip|JamieCam project/i)
  })

  it('Open Project rejects a 2d-profile project (G-code Viewer mode can\'t open it)', async () => {
    const mode2: ProjectState = {
      fileName: 'shape.svg',
      mode: '2d-profile',
      payload: { kind: '2d-profile' },
    }
    const jcamFile = new File([new Uint8Array(packJcamProject(mode2))], 'shape.jcam', {
      type: 'application/zip',
    })

    render(<ToolpathViewerMode />)
    const projectInput = screen.getByLabelText('Project file') as HTMLInputElement
    fireEvent.change(projectInput, { target: { files: [jcamFile] } })

    expect(await screen.findByRole('alert')).toHaveTextContent(/2d-profile/)
    expect(loadGcodeForViewer).not.toHaveBeenCalled()
  })

  it('Recent list appears after loading a file and restores it on click', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValue(SAMPLE_RESULT)

    render(<ToolpathViewerMode />)
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'first.nc', { type: 'text/plain' })] },
    })

    // After load, the Recent section should appear with the file in it.
    const recentBtn = await screen.findByRole('button', { name: 'first.nc' })

    // Now load a different file so we can verify clicking Recent restores
    // back to the first one.
    fireEvent.change(input, {
      target: { files: [new File(['G1 X1\n'], 'second.nc', { type: 'text/plain' })] },
    })
    await waitFor(() => expect(screen.getByText('second.nc')).toBeInTheDocument())

    vi.mocked(loadGcodeForViewer).mockClear()
    fireEvent.click(recentBtn)

    await waitFor(() => {
      expect(loadGcodeForViewer).toHaveBeenCalledWith('G0 X0\n')
    })
  })

  it('hides the Recent section when IndexedDB has no entries', async () => {
    render(<ToolpathViewerMode />)
    // No files loaded → no recents → no section header.
    await waitFor(() => {
      expect(screen.queryByText('Recent')).not.toBeInTheDocument()
    })
  })

  it('seeded recents are visible on mount', async () => {
    await upsertRecent({
      fileName: 'seeded.nc',
      mode: 'gcode-viewer',
      payload: {
        gcode: 'G0\n',
        sim: {
          stock: { origin: { x: 0, y: 0, z: 0 }, width: 50, depth: 50, height: 5 },
          toolDiameter: 3,
          resolution: 0.5,
        },
      },
    })

    render(<ToolpathViewerMode />)
    expect(await screen.findByRole('button', { name: 'seeded.nc' })).toBeInTheDocument()
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
