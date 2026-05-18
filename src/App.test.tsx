/**
 * Tests for the App shell — mode dispatch on cold start, "New Project"
 * mode picker, and `.jcam`-driven mode switching.
 *
 * The wasm-backed API and the 3-D viewport are mocked exactly as in
 * `ToolpathViewerMode.test.tsx` so these tests don't need the real
 * WebAssembly module to mount the Mode 1 surface.
 */

import { render, screen, fireEvent, waitFor, within } from '@testing-library/react'
import { IDBFactory } from 'fake-indexeddb'
import App from './App'
import { __resetRecentsForTests, upsertRecent } from './persistence/recents'
import { packJcamProject, type ProjectState } from './persistence/projectFile'
import type { GcodeViewerLoadResult } from './api/types'

// ── Mocks ─────────────────────────────────────────────────────────────────────

vi.mock('./api/gcodeViewer', () => ({
  loadGcodeForViewer: vi.fn(),
  simulateGcodeViewer: vi.fn(),
  prewarmWasm: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('./viewport/Viewport', () => ({
  Viewport: () => <div data-testid="viewport-mock" />,
}))

// Mode 2 mounts a real <canvas>; jsdom doesn't implement getContext,
// which would otherwise spam stderr from every Mode 2 mount.
vi.mock('./viewport2d/Canvas2DViewport', () => ({
  Canvas2DViewport: () => <div data-testid="canvas2d-mock" />,
}))

import { loadGcodeForViewer } from './api/gcodeViewer'

// ── Fixtures ──────────────────────────────────────────────────────────────────

const EMPTY_LOAD_RESULT: GcodeViewerLoadResult = {
  stock: null,
  tools: [],
  lineGeometry: { positions: [], colours: [], types: [] },
  warnings: [],
}

function mode1Project(fileName = 'project.nc'): ProjectState {
  return {
    fileName,
    mode: 'gcode-viewer',
    payload: {
      gcode: '; @TOOL\nG0 X0\n',
      sim: {
        stock: { origin: { x: 0, y: 0, z: 0 }, width: 50, depth: 50, height: 5 },
        toolDiameter: 3,
        resolution: 0.5,
      },
    },
  }
}

function mode2Project(fileName = 'shape.svg'): ProjectState {
  return {
    fileName,
    mode: '2d-profile',
    payload: { kind: '2d-profile' },
  }
}

function jcamFile(state: ProjectState, name = 'project.jcam'): File {
  return new File([new Uint8Array(packJcamProject(state))], name, {
    type: 'application/zip',
  })
}

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(loadGcodeForViewer).mockResolvedValue(EMPTY_LOAD_RESULT)
  globalThis.indexedDB = new IDBFactory()
  __resetRecentsForTests()
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('App shell', () => {
  it('cold-starts in Mode 1 (G-code Viewer) by default', async () => {
    render(<App />)
    // Mode 1 surface — "Open G-code…" button is its calling card.
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    // Mode 2 surface is absent.
    expect(screen.queryByTestId('mode2-root')).not.toBeInTheDocument()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('switches to Mode 2 when its "New Project" button is clicked', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: '2-D Profile' }))

    expect(screen.getByTestId('mode2-root')).toBeInTheDocument()
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
    // Let Mode 2's prewarm useEffect settle so its setState doesn't fire
    // after the test returns (would trigger an act() warning).
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('switches back to Mode 1 from Mode 2 via "New Project"', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: '2-D Profile' }))
    expect(screen.getByTestId('mode2-root')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'G-code Viewer' }))
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    expect(screen.queryByTestId('mode2-root')).not.toBeInTheDocument()
    // Let Mode 1's prewarm useEffect settle so its setState doesn't fire
    // after the test returns (would trigger an act() warning).
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('loads a Mode 1 project via the shell and renders the Mode 1 component', async () => {
    render(<App />)
    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, { target: { files: [jcamFile(mode1Project())] } })

    // App passes the unpacked project into Mode 1, which hydrates from it.
    await waitFor(() => {
      expect(loadGcodeForViewer).toHaveBeenCalledWith('; @TOOL\nG0 X0\n')
    })
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    expect(screen.queryByTestId('mode2-root')).not.toBeInTheDocument()
  })

  it('shell-opened projects appear in the Recent list', async () => {
    // Opening via the shell should match the existing in-sidebar
    // "Open Project…" semantics — the file ends up in Recents so the
    // user can return to it.
    render(<App />)
    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [jcamFile(mode1Project('shell-opened.nc'), 'shell-opened.jcam')] },
    })

    expect(
      await screen.findByRole('button', { name: 'shell-opened.nc' }),
    ).toBeInTheDocument()
  })

  it('loads a Mode 2 project via the shell and renders the Mode 2 component', async () => {
    render(<App />)
    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, { target: { files: [jcamFile(mode2Project(), 'shape.jcam')] } })

    await waitFor(() => {
      expect(screen.getByTestId('mode2-root')).toBeInTheDocument()
    })
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
    // Mode 1 mounted at cold start with no initialProject, then unmounted
    // when we switched. It must never have been asked to parse anything —
    // the Mode 2 project must not have leaked into the G-code pipeline.
    expect(loadGcodeForViewer).not.toHaveBeenCalled()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('switches from Mode 1 to Mode 2 when a Mode 2 project is opened', async () => {
    render(<App />)
    // Confirm we start in Mode 1.
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()

    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, { target: { files: [jcamFile(mode2Project(), 'shape.jcam')] } })

    await waitFor(() => {
      expect(screen.getByTestId('mode2-root')).toBeInTheDocument()
    })
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('shows an error in the header for an invalid project file', async () => {
    render(<App />)
    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['not a zip'], 'junk.jcam', { type: 'application/zip' })] },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent(/valid zip|JamieCam project/i)
    // Still in Mode 1; mode did not switch on a bad file.
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
  })

  it('Save Project starts disabled and enables after Mode 1 emits state', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce({
      stock: {
        stockType: 'box',
        width: 100,
        depth: 50,
        height: 10,
        origin: { x: 0, y: 0, z: 0 },
      },
      tools: [
        { number: 1, toolType: 'flat_endmill', diameter: 6, flutes: 4, material: null },
      ],
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings: [],
    })

    render(<App />)
    const save = screen.getByRole('button', { name: 'Save Project' })
    expect(save).toBeDisabled()

    // Loading a .nc with metadata makes Mode 1 emit a savable state.
    const input = screen.getByLabelText('G-code file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['G0 X0\n'], 'p.nc', { type: 'text/plain' })] },
    })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Save Project' })).not.toBeDisabled()
    })
  })

  it('Save Project downloads a .jcam blob built from the current state', async () => {
    vi.mocked(loadGcodeForViewer).mockResolvedValueOnce({
      stock: {
        stockType: 'box',
        width: 100,
        depth: 50,
        height: 10,
        origin: { x: 0, y: 0, z: 0 },
      },
      tools: [
        { number: 1, toolType: 'flat_endmill', diameter: 6, flutes: 4, material: null },
      ],
      lineGeometry: { positions: [], colours: [], types: [] },
      warnings: [],
    })

    const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:fake')
    const revokeObjectURL = vi.fn<(url: string) => void>()
    vi.stubGlobal('URL', { ...URL, createObjectURL, revokeObjectURL })
    const anchorClick = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => {})

    try {
      render(<App />)
      const input = screen.getByLabelText('G-code file') as HTMLInputElement
      fireEvent.change(input, {
        target: { files: [new File(['G0 X0\n'], 'pocket.nc', { type: 'text/plain' })] },
      })
      await waitFor(() => {
        expect(screen.getByRole('button', { name: 'Save Project' })).not.toBeDisabled()
      })

      fireEvent.click(screen.getByRole('button', { name: 'Save Project' }))

      expect(createObjectURL).toHaveBeenCalledTimes(1)
      const blob = createObjectURL.mock.calls[0][0]
      expect(blob.type).toBe('application/zip')
      expect(blob.size).toBeGreaterThan(0)
      expect(anchorClick).toHaveBeenCalledTimes(1)
      expect(revokeObjectURL).toHaveBeenCalledWith('blob:fake')
    } finally {
      anchorClick.mockRestore()
      vi.unstubAllGlobals()
    }
  })

  it('Mode 2 file open shows the file in Recents with the 2D badge', async () => {
    render(<App />)
    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [jcamFile(mode2Project('shape.svg'), 'shape.jcam')] },
    })

    // The Recent entry shows up keyed by the source file name.
    const recentBtn = await screen.findByRole('button', { name: 'shape.svg' })
    // …with the 2D mode badge alongside it.
    const badge = within(recentBtn).getByTestId('recent-mode-badge')
    expect(badge).toHaveTextContent('2D')
  })

  it('Recents shows Mode 1 and Mode 2 entries side by side with badges', async () => {
    // Seed IndexedDB directly so we don't need to flow through the
    // mode components — the shell reads Recents on mount.
    await upsertRecent(mode1Project('a.nc'), 1000)
    await upsertRecent(mode2Project('b.svg'), 2000)

    render(<App />)

    const list = await screen.findByRole('list', { name: 'Recent projects' })
    const items = within(list).getAllByRole('listitem')
    expect(items).toHaveLength(2)

    // Newest first: b.svg (Mode 2) then a.nc (Mode 1).
    expect(within(items[0]).getByRole('button', { name: 'b.svg' })).toBeInTheDocument()
    expect(within(items[0]).getByTestId('recent-mode-badge')).toHaveTextContent('2D')
    expect(within(items[1]).getByRole('button', { name: 'a.nc' })).toBeInTheDocument()
    expect(within(items[1]).getByTestId('recent-mode-badge')).toHaveTextContent('GC')
  })

  it('clicking a Mode 1 recent restores Mode 1; clicking a Mode 2 recent switches to Mode 2', async () => {
    await upsertRecent(mode1Project('legacy.nc'), 1000)
    await upsertRecent(mode2Project('shape.svg'), 2000)

    render(<App />)

    // Click Mode 2 recent first — shell switches modes.
    fireEvent.click(await screen.findByRole('button', { name: 'shape.svg' }))
    await waitFor(() => {
      expect(screen.getByTestId('mode2-root')).toBeInTheDocument()
    })
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()

    // Click Mode 1 recent — shell switches back and Mode 1 loads the file.
    fireEvent.click(await screen.findByRole('button', { name: 'legacy.nc' }))
    await waitFor(() => {
      expect(loadGcodeForViewer).toHaveBeenCalledWith('; @TOOL\nG0 X0\n')
    })
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    expect(screen.queryByTestId('mode2-root')).not.toBeInTheDocument()
  })
})
