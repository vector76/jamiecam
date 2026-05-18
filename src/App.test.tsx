/**
 * Tests for the App shell — mode dispatch on cold start, "New Project"
 * mode picker, and `.jcam`-driven mode switching.
 *
 * The wasm-backed API and the 3-D viewport are mocked exactly as in
 * `ToolpathViewerMode.test.tsx` so these tests don't need the real
 * WebAssembly module to mount the Mode 1 surface.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { IDBFactory } from 'fake-indexeddb'
import App from './App'
import { __resetRecentsForTests } from './persistence/recents'
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
    // Mode 2 placeholder is absent.
    expect(screen.queryByTestId('mode2-placeholder')).not.toBeInTheDocument()
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
  })

  it('switches to Mode 2 when its "New Project" button is clicked', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: '2-D Profile' }))

    expect(screen.getByTestId('mode2-placeholder')).toBeInTheDocument()
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
  })

  it('switches back to Mode 1 from Mode 2 via "New Project"', async () => {
    render(<App />)
    fireEvent.click(screen.getByRole('button', { name: '2-D Profile' }))
    expect(screen.getByTestId('mode2-placeholder')).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: 'G-code Viewer' }))
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()
    expect(screen.queryByTestId('mode2-placeholder')).not.toBeInTheDocument()
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
    expect(screen.queryByTestId('mode2-placeholder')).not.toBeInTheDocument()
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
      expect(screen.getByTestId('mode2-placeholder')).toBeInTheDocument()
    })
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
    // Mode 1 mounted at cold start with no initialProject, then unmounted
    // when we switched. It must never have been asked to parse anything —
    // the Mode 2 project must not have leaked into the G-code pipeline.
    expect(loadGcodeForViewer).not.toHaveBeenCalled()
  })

  it('switches from Mode 1 to Mode 2 when a Mode 2 project is opened', async () => {
    render(<App />)
    // Confirm we start in Mode 1.
    expect(screen.getByText('Open G-code…')).toBeInTheDocument()

    const input = screen.getByLabelText('Shell project file') as HTMLInputElement
    fireEvent.change(input, { target: { files: [jcamFile(mode2Project(), 'shape.jcam')] } })

    await waitFor(() => {
      expect(screen.getByTestId('mode2-placeholder')).toBeInTheDocument()
    })
    expect(screen.queryByText('Open G-code…')).not.toBeInTheDocument()
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
})
