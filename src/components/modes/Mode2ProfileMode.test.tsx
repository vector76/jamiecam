/**
 * Tests for Mode2ProfileMode — the Phase 4 (2-D Profile Cuts) shell.
 *
 * The wasm bridge, the Mode 2 parser API, and the Canvas2D viewport are
 * mocked so we can verify layout, file picker plumbing, parser-error
 * surfacing, and the Paths selection list without booting WebAssembly
 * or a real canvas.
 */

import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { IDBFactory } from 'fake-indexeddb'
import { Mode2ProfileMode } from './Mode2ProfileMode'
import { useViewport2DStore } from '../../store/viewport2dStore'
import { __resetDBForTests } from '../../persistence/db'
import {
  loadActiveSetupId,
  saveActiveSetupId,
  saveWorkingEnv,
} from '../../persistence/workingEnv'
import type {
  AppError,
  MachineSetup,
  MeshData,
  ParseDxfResult,
  ParseSvgResult,
  Polyline,
  ProfileOperationInput,
  Tool,
  ToolpathOutput,
  WorkingEnvironment,
} from '../../api/types'

vi.mock('../../api/gcodeViewer', () => ({
  prewarmWasm: vi.fn().mockResolvedValue(undefined),
  simulateGcodeViewer: vi.fn(),
}))

vi.mock('../../api/mode2', () => ({
  parseSvg: vi.fn(),
  parseDxf: vi.fn(),
  generateProfileToolpath: vi.fn(),
  emitGrblGcode: vi.fn(),
}))

vi.mock('../../viewport2d/Canvas2DViewport', () => ({
  Canvas2DViewport: () => <div data-testid="canvas2d-mock" />,
}))

vi.mock('../../viewport/Viewport', () => ({
  Viewport: () => <div data-testid="viewport3d-mock" />,
}))

import { prewarmWasm, simulateGcodeViewer } from '../../api/gcodeViewer'
import { useViewportStore } from '../../store/viewportStore'
import {
  emitGrblGcode,
  generateProfileToolpath,
  parseDxf,
  parseSvg,
} from '../../api/mode2'

const SQUARE: Polyline = {
  closed: true,
  points: [
    { x: 0, y: 0 },
    { x: 10, y: 0 },
    { x: 10, y: 10 },
    { x: 0, y: 10 },
  ],
}

const LINE: Polyline = {
  closed: false,
  points: [
    { x: 0, y: 0 },
    { x: 5, y: 5 },
  ],
}

beforeEach(() => {
  vi.clearAllMocks()
  useViewport2DStore.getState().reset()
  useViewportStore.setState({ simulationMeshData: null })
  // Fresh in-memory DB per test so the loaded working-environment state
  // doesn't bleed between tests (the component reads it on mount).
  globalThis.indexedDB = new IDBFactory()
  __resetDBForTests()
})

function makeSetup(id: string, name = `Setup ${id}`): MachineSetup {
  return {
    id,
    name,
    workspace: { origin: { x: 0, y: 0, z: 0 }, width: 300, depth: 200, height: 80 },
    kinematics: '3-axis-router',
    postProcessor: 'grbl-1.1',
    safety: { safeZ: 5, rapidFeedRate: 3000 },
  }
}

function makeTool(id: string, name = `Tool ${id}`): Tool {
  return {
    id,
    name,
    diameter: 3.175,
    fluteCount: 2,
    length: 38,
    material: 'carbide',
    recommended: { spindleRpm: 18000, feedRate: 800, plungeRate: 200 },
  }
}

async function seedEnv(env: WorkingEnvironment): Promise<void> {
  await saveWorkingEnv(env)
}

async function waitForReady() {
  await waitFor(() => {
    expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
  })
}

describe('Mode2ProfileMode', () => {
  it('renders the Canvas2D workspace alongside the sidebar sections', async () => {
    render(<Mode2ProfileMode />)

    expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()

    for (const title of ['File', 'Paths', 'Operation', 'Generate', 'Simulate', 'Export']) {
      expect(screen.getByRole('button', { name: title })).toBeInTheDocument()
    }

    await waitForReady()
  })

  it('shows an engine-initializing indicator until prewarm resolves', async () => {
    let resolvePrewarm!: () => void
    vi.mocked(prewarmWasm).mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolvePrewarm = resolve
      }),
    )

    render(<Mode2ProfileMode />)
    expect(screen.getByText('Initializing engine…')).toBeInTheDocument()

    resolvePrewarm()
    await waitForReady()
  })

  it('surfaces an error if the wasm engine fails to initialize', async () => {
    vi.mocked(prewarmWasm).mockRejectedValueOnce({
      kind: 'Io',
      message: 'failed to fetch wasm',
    })

    render(<Mode2ProfileMode />)

    await waitFor(() => {
      expect(
        screen.getByText(/Engine failed to load: failed to fetch wasm/),
      ).toBeInTheDocument()
    })
  })

  it('exposes a hidden file input that accepts .svg and .dxf', async () => {
    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    expect(input.type).toBe('file')
    expect(input.accept).toBe('.svg,.dxf')
    expect(input.multiple).toBe(false)
    await waitForReady()
  })

  it('routes a .svg file to parseSvg and lists the imported paths', async () => {
    const result: ParseSvgResult = { paths: [SQUARE, LINE], warnings: [] }
    vi.mocked(parseSvg).mockResolvedValueOnce(result)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'shape.svg', { type: 'image/svg+xml' })] },
    })

    await waitFor(() => {
      expect(parseSvg).toHaveBeenCalledTimes(1)
      expect(parseDxf).not.toHaveBeenCalled()
    })

    expect(await screen.findByText('shape.svg')).toBeInTheDocument()
    expect(screen.getByLabelText('Path 1')).toBeInTheDocument()
    expect(screen.getByLabelText('Path 2')).toBeInTheDocument()
    expect(screen.getByText(/closed · 4 pts/)).toBeInTheDocument()
    expect(screen.getByText(/open · 2 pts/)).toBeInTheDocument()
  })

  it('routes a .dxf file to parseDxf', async () => {
    const result: ParseDxfResult = { paths: [LINE], warnings: [] }
    vi.mocked(parseDxf).mockResolvedValueOnce(result)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['0\nEOF\n'], 'part.dxf', { type: 'application/dxf' })] },
    })

    await waitFor(() => {
      expect(parseDxf).toHaveBeenCalledTimes(1)
      expect(parseSvg).not.toHaveBeenCalled()
    })
    expect(await screen.findByText('part.dxf')).toBeInTheDocument()
  })

  it('rejects an unsupported file extension with an inline error', async () => {
    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['x'], 'photo.png', { type: 'image/png' })] },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent(
      /Unsupported file type: photo\.png/,
    )
    expect(parseSvg).not.toHaveBeenCalled()
    expect(parseDxf).not.toHaveBeenCalled()
  })

  it('surfaces a ParseFailure AppError as a red alert', async () => {
    const failure: AppError = {
      kind: 'ParseFailure',
      message: { source: 'svg', message: 'mismatched tag', line: 12 },
    }
    vi.mocked(parseSvg).mockRejectedValueOnce(failure)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg'], 'broken.svg', { type: 'image/svg+xml' })] },
    })

    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent(/svg: mismatched tag \(line 12\)/)
  })

  it('falls back to message-or-kind for non-ParseFailure AppErrors', async () => {
    vi.mocked(parseDxf).mockRejectedValueOnce({
      kind: 'InvalidInput',
      message: 'no entities',
    } satisfies AppError)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['0'], 'empty.dxf', { type: 'application/dxf' })] },
    })

    expect(await screen.findByRole('alert')).toHaveTextContent('no entities')
  })

  it('renders ParseWarnings as a yellow inline list', async () => {
    const result: ParseSvgResult = {
      paths: [SQUARE],
      warnings: [
        { line: 7, message: 'skipped <text>' },
        { line: null, message: 'unsupported transform' },
      ],
    }
    vi.mocked(parseSvg).mockResolvedValueOnce(result)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'warned.svg', { type: 'image/svg+xml' })] },
    })

    const skipped = await screen.findByText(/skipped <text>/)
    expect(skipped).toHaveTextContent('(line 7)')
    expect(skipped.className).toMatch(/text-yellow-600/)
    expect(screen.getByText(/unsupported transform/)).toBeInTheDocument()
  })

  it('toggles the per-path checkbox without affecting other rows', async () => {
    const result: ParseSvgResult = { paths: [SQUARE, LINE], warnings: [] }
    vi.mocked(parseSvg).mockResolvedValueOnce(result)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'pair.svg', { type: 'image/svg+xml' })] },
    })

    const p1 = (await screen.findByLabelText('Path 1')) as HTMLInputElement
    const p2 = screen.getByLabelText('Path 2') as HTMLInputElement
    expect(p1.checked).toBe(true)
    expect(p2.checked).toBe(true)

    fireEvent.click(p1)
    expect(p1.checked).toBe(false)
    expect(p2.checked).toBe(true)
  })

  it('clears stale warnings when a new import succeeds', async () => {
    vi.mocked(parseSvg)
      .mockResolvedValueOnce({
        paths: [SQUARE],
        warnings: [{ line: null, message: 'old warning' }],
      })
      .mockResolvedValueOnce({ paths: [LINE], warnings: [] })

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'a.svg', { type: 'image/svg+xml' })] },
    })
    await screen.findByText(/old warning/)

    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'b.svg', { type: 'image/svg+xml' })] },
    })
    await waitFor(() => {
      expect(screen.queryByText(/old warning/)).not.toBeInTheDocument()
    })
  })

  it('clears a stale engine-failure indicator once an import succeeds', async () => {
    vi.mocked(prewarmWasm).mockRejectedValueOnce({
      kind: 'Io',
      message: 'flaky network',
    })
    vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [SQUARE], warnings: [] })

    render(<Mode2ProfileMode />)
    await screen.findByText(/Engine failed to load: flaky network/)

    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 'ok.svg', { type: 'image/svg+xml' })] },
    })

    await waitFor(() => {
      expect(screen.queryByText(/Engine failed to load/)).not.toBeInTheDocument()
    })
  })

  it('exposes a Load Sample dropdown with bundled SVG and DXF samples', async () => {
    render(<Mode2ProfileMode />)
    const select = screen.getByLabelText('Load Sample') as HTMLSelectElement
    expect(select.tagName).toBe('SELECT')
    const optionValues = Array.from(select.options).map((o) => o.value)
    expect(optionValues).toContain('sample-profile.svg')
    expect(optionValues).toContain('sample-profile.dxf')
    await waitForReady()
  })

  it('fetches and parses the SVG sample when chosen', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(new Response('<svg/>', { status: 200 }))
    vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [SQUARE], warnings: [] })

    render(<Mode2ProfileMode />)
    const select = screen.getByLabelText('Load Sample') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'sample-profile.svg' } })

    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalledWith(
        expect.stringContaining('samples/sample-profile.svg'),
      )
      expect(parseSvg).toHaveBeenCalledTimes(1)
      expect(parseDxf).not.toHaveBeenCalled()
    })

    expect(await screen.findByText('sample-profile.svg')).toBeInTheDocument()
    expect(screen.getByLabelText('Path 1')).toBeInTheDocument()
  })

  it('fetches and parses the DXF sample when chosen', async () => {
    const fetchSpy = vi
      .spyOn(globalThis, 'fetch')
      .mockResolvedValueOnce(new Response('0\nEOF\n', { status: 200 }))
    vi.mocked(parseDxf).mockResolvedValueOnce({ paths: [LINE], warnings: [] })

    render(<Mode2ProfileMode />)
    const select = screen.getByLabelText('Load Sample') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'sample-profile.dxf' } })

    await waitFor(() => {
      expect(fetchSpy).toHaveBeenCalledWith(
        expect.stringContaining('samples/sample-profile.dxf'),
      )
      expect(parseDxf).toHaveBeenCalledTimes(1)
      expect(parseSvg).not.toHaveBeenCalled()
    })

    expect(await screen.findByText('sample-profile.dxf')).toBeInTheDocument()
  })

  it('surfaces a sample fetch failure as a red alert', async () => {
    vi.spyOn(globalThis, 'fetch').mockResolvedValueOnce(
      new Response('not found', { status: 404 }),
    )

    render(<Mode2ProfileMode />)
    const select = screen.getByLabelText('Load Sample') as HTMLSelectElement
    fireEvent.change(select, { target: { value: 'sample-profile.svg' } })

    expect(await screen.findByRole('alert')).toHaveTextContent(/Sample fetch failed: 404/)
    expect(parseSvg).not.toHaveBeenCalled()
  })

  it('updates the viewport store extent to the imported paths bounding box', async () => {
    const result: ParseSvgResult = { paths: [SQUARE], warnings: [] }
    vi.mocked(parseSvg).mockResolvedValueOnce(result)

    render(<Mode2ProfileMode />)
    const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
    fireEvent.change(input, {
      target: { files: [new File(['<svg/>'], 's.svg', { type: 'image/svg+xml' })] },
    })

    await waitFor(() => {
      expect(useViewport2DStore.getState().extent).toEqual({
        minX: 0,
        minY: 0,
        maxX: 10,
        maxY: 10,
      })
    })
  })

  describe('Setup section', () => {
    it('shows an empty-state message when no setups are configured', async () => {
      render(<Mode2ProfileMode />)
      expect(
        await screen.findByText(/No machine setups configured/),
      ).toBeInTheDocument()
      expect(screen.queryByLabelText('Active machine setup')).not.toBeInTheDocument()
      await waitForReady()
    })

    it('lists persisted setups in the active-setup selector', async () => {
      await seedEnv({
        setups: [makeSetup('s1', 'Workshop CNC'), makeSetup('s2', 'Garage CNC')],
        tools: [],
        availability: [],
      })
      await saveActiveSetupId('s2')

      render(<Mode2ProfileMode />)

      const select = (await screen.findByLabelText(
        'Active machine setup',
      )) as HTMLSelectElement
      const labels = Array.from(select.options).map((o) => o.textContent)
      expect(labels).toContain('Workshop CNC')
      expect(labels).toContain('Garage CNC')
      expect(select.value).toBe('s2')
    })

    it('falls back to the first setup when the persisted active id is stale', async () => {
      await seedEnv({
        setups: [makeSetup('s1'), makeSetup('s2')],
        tools: [],
        availability: [],
      })
      // Simulate stale state — e.g. another tab deleted the setup we
      // were active on, leaving the orphan id in IDB.
      await saveActiveSetupId('s-ghost')

      render(<Mode2ProfileMode />)

      const select = (await screen.findByLabelText(
        'Active machine setup',
      )) as HTMLSelectElement
      await waitFor(() => expect(select.value).toBe('s1'))
    })

    it('defaults the active setup to the first one if none is persisted', async () => {
      await seedEnv({
        setups: [makeSetup('s1'), makeSetup('s2')],
        tools: [],
        availability: [],
      })

      render(<Mode2ProfileMode />)

      const select = (await screen.findByLabelText(
        'Active machine setup',
      )) as HTMLSelectElement
      await waitFor(() => expect(select.value).toBe('s1'))
    })

    it('persists a new active-setup choice', async () => {
      await seedEnv({
        setups: [makeSetup('s1'), makeSetup('s2')],
        tools: [],
        availability: [],
      })
      await saveActiveSetupId('s1')

      render(<Mode2ProfileMode />)

      const select = (await screen.findByLabelText(
        'Active machine setup',
      )) as HTMLSelectElement
      fireEvent.change(select, { target: { value: 's2' } })

      await waitFor(async () => {
        expect(await loadActiveSetupId()).toBe('s2')
      })
    })

    it('re-reads the working environment after the modal closes', async () => {
      render(<Mode2ProfileMode />)
      // First-run: nothing seeded yet, so the empty-state message renders.
      expect(
        await screen.findByText(/No machine setups configured/),
      ).toBeInTheDocument()

      // Open the modal and wait for its own internal load to settle so we
      // don't leak a setState past the test's assertions.
      fireEvent.click(screen.getByRole('button', { name: /Working Environment…/ }))
      await screen.findByRole('button', { name: /add setup/i })

      // Simulate the modal having added a setup by writing directly to IDB
      // (the parent doesn't care which UI mutated the store, only that it
      // re-reads after onClose fires).
      await saveWorkingEnv({
        setups: [makeSetup('s1', 'Just Added')],
        tools: [],
        availability: [],
      })

      // Close the modal — the parent's onClose handler must trigger a refresh.
      fireEvent.click(screen.getByRole('button', { name: /close/i }))

      const select = (await screen.findByLabelText(
        'Active machine setup',
      )) as HTMLSelectElement
      await waitFor(() => expect(select.value).toBe('s1'))
      expect(
        Array.from(select.options).map((o) => o.textContent),
      ).toContain('Just Added')
    })
  })

  describe('Operation form', () => {
    async function renderWithEnv(env: WorkingEnvironment, activeId: string | null) {
      await seedEnv(env)
      if (activeId !== null) await saveActiveSetupId(activeId)
      const result = render(<Mode2ProfileMode />)
      // Wait until the working-environment load resolves so subsequent
      // queries see the tool dropdown / form fields.
      await waitForReady()
      return result
    }

    it('shows a prompt when no setup is active', async () => {
      render(<Mode2ProfileMode />)
      expect(
        await screen.findByText(/Choose an active setup to see its tools/),
      ).toBeInTheDocument()
      await waitForReady()
    })

    it('only lists tools whose availability pair matches the active setup', async () => {
      await renderWithEnv(
        {
          setups: [makeSetup('s1'), makeSetup('s2')],
          tools: [
            makeTool('t1', '1/8" end mill'),
            makeTool('t2', '1/4" end mill'),
            makeTool('t3', '60° V-bit'),
          ],
          availability: [
            { setupId: 's1', toolId: 't1' },
            { setupId: 's1', toolId: 't3' },
            { setupId: 's2', toolId: 't2' },
          ],
        },
        's1',
      )

      const toolSelect = (await screen.findByLabelText('Tool')) as HTMLSelectElement
      const labels = Array.from(toolSelect.options)
        .map((o) => o.textContent)
        .filter((l) => l && !l.startsWith('Choose'))
      expect(labels).toEqual(['1/8" end mill', '60° V-bit'])
    })

    it('selects the first available tool by default', async () => {
      await renderWithEnv(
        {
          setups: [makeSetup('s1')],
          tools: [makeTool('t1', 'A'), makeTool('t2', 'B')],
          availability: [
            { setupId: 's1', toolId: 't1' },
            { setupId: 's1', toolId: 't2' },
          ],
        },
        's1',
      )

      const toolSelect = (await screen.findByLabelText('Tool')) as HTMLSelectElement
      await waitFor(() => expect(toolSelect.value).toBe('t1'))
    })

    it('re-snaps the tool selection when the active setup changes', async () => {
      await renderWithEnv(
        {
          setups: [makeSetup('s1'), makeSetup('s2')],
          tools: [makeTool('t1', 'A'), makeTool('t2', 'B')],
          availability: [
            { setupId: 's1', toolId: 't1' },
            { setupId: 's2', toolId: 't2' },
          ],
        },
        's1',
      )

      const toolSelect = (await screen.findByLabelText('Tool')) as HTMLSelectElement
      await waitFor(() => expect(toolSelect.value).toBe('t1'))

      const setupSelect = screen.getByLabelText('Active machine setup') as HTMLSelectElement
      fireEvent.change(setupSelect, { target: { value: 's2' } })

      await waitFor(() => {
        const refreshedTool = screen.getByLabelText('Tool') as HTMLSelectElement
        expect(refreshedTool.value).toBe('t2')
      })
    })

    it('reports an empty-tool-list state when the active setup has no compatible tools', async () => {
      await renderWithEnv(
        {
          setups: [makeSetup('s1')],
          tools: [makeTool('t1')],
          availability: [],
        },
        's1',
      )

      expect(
        await screen.findByText(/No tools available for this setup/),
      ).toBeInTheDocument()
      expect(screen.queryByLabelText('Tool')).not.toBeInTheDocument()
    })

    it('renders the three cut-side options with Outside selected by default', async () => {
      render(<Mode2ProfileMode />)
      const fieldset = (await screen.findByRole('group', {
        name: 'Cut side',
      })) as HTMLFieldSetElement

      const outside = within(fieldset).getByLabelText('Outside') as HTMLInputElement
      const inside = within(fieldset).getByLabelText('Inside') as HTMLInputElement
      const onLine = within(fieldset).getByLabelText('On Line') as HTMLInputElement

      expect(outside.checked).toBe(true)
      expect(inside.checked).toBe(false)
      expect(onLine.checked).toBe(false)
      await waitForReady()
    })

    it('toggles cut-side between Outside, Inside, and On Line', async () => {
      render(<Mode2ProfileMode />)
      const inside = (await screen.findByLabelText('Inside')) as HTMLInputElement
      const onLine = screen.getByLabelText('On Line') as HTMLInputElement
      const outside = screen.getByLabelText('Outside') as HTMLInputElement

      fireEvent.click(inside)
      expect(inside.checked).toBe(true)
      expect(outside.checked).toBe(false)
      expect(onLine.checked).toBe(false)

      fireEvent.click(onLine)
      expect(onLine.checked).toBe(true)
      expect(inside.checked).toBe(false)
      await waitForReady()
    })

    it('exposes the six numeric inputs with sensible defaults', async () => {
      render(<Mode2ProfileMode />)
      const fields: Array<[string, string]> = [
        ['Depth total', '5'],
        ['Depth per pass', '1'],
        ['Safe Z', '5'],
        ['Plunge feed', '200'],
        ['Cut feed', '800'],
        ['Spindle RPM', '18000'],
      ]
      for (const [label, defaultValue] of fields) {
        const input = (await screen.findByLabelText(label)) as HTMLInputElement
        expect(input.type).toBe('number')
        expect(input.value).toBe(defaultValue)
      }
      await waitForReady()
    })

    it('updates a numeric field when edited', async () => {
      render(<Mode2ProfileMode />)
      const depthTotal = (await screen.findByLabelText(
        'Depth total',
      )) as HTMLInputElement
      fireEvent.change(depthTotal, { target: { value: '12.5' } })
      expect(depthTotal.value).toBe('12.5')

      const rpm = screen.getByLabelText('Spindle RPM') as HTMLInputElement
      fireEvent.change(rpm, { target: { value: '24000' } })
      expect(rpm.value).toBe('24000')
      await waitForReady()
    })
  })

  describe('Generate', () => {
    const STOCK_TOOL = makeTool('t1', '1/8" end mill')
    const READY_ENV: WorkingEnvironment = {
      setups: [makeSetup('s1')],
      tools: [STOCK_TOOL],
      availability: [{ setupId: 's1', toolId: 't1' }],
    }

    async function renderReady(paths: Polyline[] = [SQUARE]) {
      await seedEnv(READY_ENV)
      await saveActiveSetupId('s1')
      vi.mocked(parseSvg).mockResolvedValueOnce({ paths, warnings: [] })

      render(<Mode2ProfileMode />)
      const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
      fireEvent.change(input, {
        target: { files: [new File(['<svg/>'], 'p.svg', { type: 'image/svg+xml' })] },
      })
      // Wait until both the working-env load and the import settle so the
      // tool dropdown has resolved and the Path rows are present.
      await screen.findByLabelText('Path 1')
      await waitFor(() => {
        const sel = screen.getByLabelText('Tool') as HTMLSelectElement
        expect(sel.value).toBe('t1')
      })
    }

    it('disables the Generate button until paths and a tool are available', async () => {
      render(<Mode2ProfileMode />)
      const btn = (await screen.findByRole('button', {
        name: 'Generate toolpath',
      })) as HTMLButtonElement
      expect(btn.disabled).toBe(true)
      expect(
        screen.getByText(/Select at least one path to generate a toolpath/),
      ).toBeInTheDocument()
      await waitForReady()
    })

    it('calls generateProfileToolpath with the selected paths, tool, and operation params', async () => {
      const motions: ToolpathOutput = [
        { kind: 'rapid', to: [0, 0, 5] },
        { kind: 'linear', to: [0, 0, -1.5], feed: 200 },
        { kind: 'linear', to: [10, 0, -1.5], feed: 800 },
        { kind: 'rapid', to: [10, 0, 5] },
      ]
      vi.mocked(generateProfileToolpath).mockResolvedValueOnce(motions)

      await renderReady([SQUARE, LINE])

      // Deselect the second path so we can confirm the boundary subset
      // makes it into the planner input intact.
      fireEvent.click(screen.getByLabelText('Path 2'))
      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))

      await waitFor(() => {
        expect(generateProfileToolpath).toHaveBeenCalledTimes(1)
      })
      const arg = vi.mocked(generateProfileToolpath).mock.calls[0][0] as ProfileOperationInput
      expect(arg.boundaries).toEqual([SQUARE])
      expect(arg.tool.id).toBe('t1')
      expect(arg.cutSide).toBe('outside')
      expect(arg.depthTotal).toBe(5)
      expect(arg.depthPerPass).toBe(1)
      expect(arg.safeZ).toBe(5)
      expect(arg.plungeFeed).toBe(200)
      expect(arg.cutFeed).toBe(800)
      expect(arg.spindleRpm).toBe(18000)

      expect(await screen.findByRole('status')).toHaveTextContent(
        /Generated 4 moves/,
      )
    })

    it('surfaces the boundary-too-small AppError as a red alert', async () => {
      const failure: AppError = {
        kind: 'InvalidInput',
        message:
          'boundary at (0.000, 0.000) is smaller than tool diameter 3.175 mm; inside cut would remove the entire shape',
      }
      vi.mocked(generateProfileToolpath).mockRejectedValueOnce(failure)

      await renderReady([SQUARE])
      fireEvent.click(screen.getByLabelText('Inside'))
      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))

      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(/smaller than tool diameter/)
      expect(alert).toHaveTextContent(/inside cut/)
      // The arg's cut-side should reflect the toggle the user made.
      const arg = vi.mocked(generateProfileToolpath).mock.calls[0][0] as ProfileOperationInput
      expect(arg.cutSide).toBe('inside')
    })

    it('clears a stale error message once a regenerate succeeds', async () => {
      vi.mocked(generateProfileToolpath)
        .mockRejectedValueOnce({
          kind: 'InvalidInput',
          message: 'first failure',
        } satisfies AppError)
        .mockResolvedValueOnce([{ kind: 'rapid', to: [0, 0, 5] }])

      await renderReady([SQUARE])
      const btn = screen.getByRole('button', { name: 'Generate toolpath' })

      fireEvent.click(btn)
      await screen.findByText(/first failure/)

      fireEvent.click(btn)
      await waitFor(() => {
        expect(screen.queryByText(/first failure/)).not.toBeInTheDocument()
      })
      expect(await screen.findByRole('status')).toHaveTextContent(
        /Generated 1 moves/,
      )
    })

    it('persists the last-generated toolpath until a new import lands', async () => {
      vi.mocked(generateProfileToolpath).mockResolvedValueOnce([
        { kind: 'rapid', to: [0, 0, 5] },
        { kind: 'linear', to: [1, 0, -1], feed: 800 },
      ])
      await renderReady([SQUARE])

      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))
      expect(await screen.findByRole('status')).toHaveTextContent(
        /Generated 2 moves/,
      )

      // Toolpath should outlive an unrelated re-render — for example,
      // toggling a path selection or editing a numeric field — so
      // downstream Simulate/Export can reuse it without regenerating.
      fireEvent.change(screen.getByLabelText('Spindle RPM'), {
        target: { value: '24000' },
      })
      expect(screen.getByRole('status')).toHaveTextContent(/Generated 2 moves/)

      // A fresh import invalidates the result — the artwork the toolpath
      // was planned against is gone.
      vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [LINE], warnings: [] })
      fireEvent.change(
        screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement,
        {
          target: {
            files: [new File(['<svg/>'], 'second.svg', { type: 'image/svg+xml' })],
          },
        },
      )
      await waitFor(() => {
        expect(screen.queryByRole('status')).not.toBeInTheDocument()
      })
    })
  })

  describe('Export', () => {
    const STOCK_TOOL = makeTool('t1', '1/8" end mill')
    const READY_ENV: WorkingEnvironment = {
      setups: [makeSetup('s1')],
      tools: [STOCK_TOOL],
      availability: [{ setupId: 's1', toolId: 't1' }],
    }
    const MOTIONS: ToolpathOutput = [
      { kind: 'rapid', to: [0, 0, 5] },
      { kind: 'linear', to: [10, 0, -1], feed: 800 },
    ]

    async function renderWithGeneratedToolpath() {
      await seedEnv(READY_ENV)
      await saveActiveSetupId('s1')
      vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [SQUARE], warnings: [] })
      vi.mocked(generateProfileToolpath).mockResolvedValueOnce(MOTIONS)

      render(<Mode2ProfileMode />)
      const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
      fireEvent.change(input, {
        target: { files: [new File(['<svg/>'], 'p.svg', { type: 'image/svg+xml' })] },
      })
      await screen.findByLabelText('Path 1')
      await waitFor(() => {
        const sel = screen.getByLabelText('Tool') as HTMLSelectElement
        expect(sel.value).toBe('t1')
      })
      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))
      await screen.findByText(/Generated 2 moves/)
    }

    it('disables the Export button until a toolpath has been generated', async () => {
      render(<Mode2ProfileMode />)
      const btn = (await screen.findByRole('button', {
        name: 'Export G-code',
      })) as HTMLButtonElement
      expect(btn.disabled).toBe(true)
      expect(
        screen.getByText(/Generate a toolpath before exporting/),
      ).toBeInTheDocument()
      await waitForReady()
    })

    it('triggers a .nc download with the emitted G-code text', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nG90\nG0 X0 Y0 Z5\nM2\n')

      const createObjectURL = vi.fn<(blob: Blob) => string>(() => 'blob:fake')
      const revokeObjectURL = vi.fn<(url: string) => void>()
      vi.stubGlobal('URL', { ...URL, createObjectURL, revokeObjectURL })
      const anchorClick = vi
        .spyOn(HTMLAnchorElement.prototype, 'click')
        .mockImplementation(() => {})

      try {
        await renderWithGeneratedToolpath()

        const btn = screen.getByRole('button', {
          name: 'Export G-code',
        }) as HTMLButtonElement
        expect(btn.disabled).toBe(false)
        fireEvent.click(btn)

        await waitFor(() => {
          expect(emitGrblGcode).toHaveBeenCalledTimes(1)
        })
        const [toolpathArg, toolArg, stockArg] = vi.mocked(emitGrblGcode).mock.calls[0]
        expect(toolpathArg).toEqual(MOTIONS)
        expect(toolArg.id).toBe('t1')
        expect(stockArg.width).toBe(300)

        await waitFor(() => {
          expect(createObjectURL).toHaveBeenCalledTimes(1)
        })
        const blob = createObjectURL.mock.calls[0][0]
        expect(blob.type).toMatch(/^text\/plain/)
        expect(blob.size).toBeGreaterThan(0)
        const downloaded = await blob.text()
        expect(downloaded).toBe('G21\nG90\nG0 X0 Y0 Z5\nM2\n')

        expect(anchorClick).toHaveBeenCalledTimes(1)
        expect(revokeObjectURL).toHaveBeenCalledWith('blob:fake')
      } finally {
        anchorClick.mockRestore()
        vi.unstubAllGlobals()
      }
    })

    it('surfaces an emitter AppError as a red alert', async () => {
      vi.mocked(emitGrblGcode).mockRejectedValueOnce({
        kind: 'InvalidInput',
        message: 'non-finite coordinate in rapid',
      } satisfies AppError)

      await renderWithGeneratedToolpath()

      fireEvent.click(screen.getByRole('button', { name: 'Export G-code' }))
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(/non-finite coordinate/)
    })
  })

  describe('Simulate', () => {
    const STOCK_TOOL = makeTool('t1', '1/8" end mill')
    const READY_ENV: WorkingEnvironment = {
      setups: [makeSetup('s1')],
      tools: [STOCK_TOOL],
      availability: [{ setupId: 's1', toolId: 't1' }],
    }
    const MOTIONS: ToolpathOutput = [
      { kind: 'rapid', to: [0, 0, 5] },
      { kind: 'linear', to: [10, 0, -1], feed: 800 },
    ]
    const SIM_MESH: MeshData = {
      vertices: [0, 0, 0, 1, 0, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2],
      faceGroups: [],
    }

    async function renderWithGeneratedToolpath() {
      await seedEnv(READY_ENV)
      await saveActiveSetupId('s1')
      vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [SQUARE], warnings: [] })
      vi.mocked(generateProfileToolpath).mockResolvedValueOnce(MOTIONS)

      render(<Mode2ProfileMode />)
      const input = screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement
      fireEvent.change(input, {
        target: { files: [new File(['<svg/>'], 'p.svg', { type: 'image/svg+xml' })] },
      })
      await screen.findByLabelText('Path 1')
      await waitFor(() => {
        const sel = screen.getByLabelText('Tool') as HTMLSelectElement
        expect(sel.value).toBe('t1')
      })
      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))
      await screen.findByText(/Generated 2 moves/)
    }

    it('disables Simulate until a toolpath has been generated', async () => {
      render(<Mode2ProfileMode />)
      const btn = (await screen.findByRole('button', {
        name: 'Simulate toolpath',
      })) as HTMLButtonElement
      expect(btn.disabled).toBe(true)
      expect(
        screen.getByText(/Generate a toolpath before simulating/),
      ).toBeInTheDocument()
      await waitForReady()
    })

    it('emits G-code, runs the dexel sim, and swaps in the 3-D viewport', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nG90\nG0 X0 Y0 Z5\nM2\n')
      vi.mocked(simulateGcodeViewer).mockResolvedValueOnce(SIM_MESH)

      await renderWithGeneratedToolpath()
      expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()

      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))

      await waitFor(() => {
        expect(emitGrblGcode).toHaveBeenCalledTimes(1)
        expect(simulateGcodeViewer).toHaveBeenCalledTimes(1)
      })

      const [emittedToolpath, emittedTool, emittedStock] = vi.mocked(emitGrblGcode)
        .mock.calls[0]
      expect(emittedToolpath).toEqual(MOTIONS)
      expect(emittedTool.id).toBe('t1')
      expect(emittedStock.width).toBe(300)

      const [simContent, simParams] = vi.mocked(simulateGcodeViewer).mock.calls[0]
      expect(simContent).toBe('G21\nG90\nG0 X0 Y0 Z5\nM2\n')
      expect(simParams.stock.width).toBe(300)
      expect(simParams.toolDiameter).toBe(STOCK_TOOL.diameter)
      expect(simParams.resolution).toBeGreaterThan(0)

      await waitFor(() => {
        expect(screen.getByTestId('viewport3d-mock')).toBeInTheDocument()
      })
      expect(screen.queryByTestId('canvas2d-mock')).not.toBeInTheDocument()
      expect(useViewportStore.getState().simulationMeshData).toBe(SIM_MESH)
    })

    it('surfaces a worker AppError as a red alert and stays in 2-D', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nM2\n')
      vi.mocked(simulateGcodeViewer).mockRejectedValueOnce({
        kind: 'WorkerError',
        message: 'simulation worker crashed',
      } satisfies AppError)

      await renderWithGeneratedToolpath()
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))

      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(/simulation worker crashed/)
      expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()
      expect(screen.queryByTestId('viewport3d-mock')).not.toBeInTheDocument()
      expect(useViewportStore.getState().simulationMeshData).toBeNull()
    })

    it('keeps the prior simulation mesh when a retry from 3-D fails', async () => {
      // First sim succeeds and lands the user in 3-D.
      vi.mocked(emitGrblGcode)
        .mockResolvedValueOnce('G21\nM2\n')
        .mockResolvedValueOnce('G21\nM2\n')
      vi.mocked(simulateGcodeViewer)
        .mockResolvedValueOnce(SIM_MESH)
        .mockRejectedValueOnce({
          kind: 'WorkerError',
          message: 'worker died mid-retry',
        } satisfies AppError)

      await renderWithGeneratedToolpath()
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))
      await screen.findByTestId('viewport3d-mock')
      expect(useViewportStore.getState().simulationMeshData).toBe(SIM_MESH)

      // Retry from 3-D — the second simulate rejects.
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))
      const alert = await screen.findByRole('alert')
      expect(alert).toHaveTextContent(/worker died mid-retry/)

      // The user should still see the original good mesh, not an empty 3-D scene.
      expect(useViewportStore.getState().simulationMeshData).toBe(SIM_MESH)
      expect(screen.getByTestId('viewport3d-mock')).toBeInTheDocument()
    })

    it('returns to the 2-D viewport without losing the toolpath state', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nM2\n')
      vi.mocked(simulateGcodeViewer).mockResolvedValueOnce(SIM_MESH)

      await renderWithGeneratedToolpath()
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))
      await screen.findByTestId('viewport3d-mock')

      fireEvent.click(screen.getByRole('button', { name: 'Back to 2D' }))

      await waitFor(() => {
        expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()
      })
      expect(screen.queryByTestId('viewport3d-mock')).not.toBeInTheDocument()
      // The "Generated N moves" status is driven off component-state
      // `toolpath`; if switching back wiped it the status would vanish
      // and Export would re-disable.
      const statuses = screen.getAllByRole('status').map((el) => el.textContent)
      expect(statuses).toEqual(
        expect.arrayContaining([expect.stringMatching(/Generated 2 moves/)]),
      )
      const exportBtn = screen.getByRole('button', {
        name: 'Export G-code',
      }) as HTMLButtonElement
      expect(exportBtn.disabled).toBe(false)
    })

    it('flips back to 2-D when the user regenerates while previewing in 3-D', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nM2\n')
      vi.mocked(simulateGcodeViewer).mockResolvedValueOnce(SIM_MESH)

      await renderWithGeneratedToolpath()
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))
      await screen.findByTestId('viewport3d-mock')

      // Queue the regenerate's planner response only now — renderWith…
      // already consumed the first slot for the initial Generate.
      vi.mocked(generateProfileToolpath).mockResolvedValueOnce([
        { kind: 'rapid', to: [0, 0, 5] },
      ])
      fireEvent.click(screen.getByRole('button', { name: 'Generate toolpath' }))

      await waitFor(() => {
        expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()
      })
      expect(useViewportStore.getState().simulationMeshData).toBeNull()
    })

    it('disables Generate while a simulation is in flight', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nM2\n')
      let resolveSim!: (mesh: MeshData) => void
      vi.mocked(simulateGcodeViewer).mockReturnValueOnce(
        new Promise<MeshData>((resolve) => {
          resolveSim = resolve
        }),
      )

      await renderWithGeneratedToolpath()
      const generateBtn = screen.getByRole('button', {
        name: 'Generate toolpath',
      }) as HTMLButtonElement
      expect(generateBtn.disabled).toBe(false)

      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))

      // Wait for the in-flight sim to register so Generate flips disabled.
      await waitFor(() => expect(generateBtn.disabled).toBe(true))

      resolveSim(SIM_MESH)
      await waitFor(() => expect(generateBtn.disabled).toBe(false))
    })

    it('clears the simulation mesh and returns to 2-D when a fresh import lands', async () => {
      vi.mocked(emitGrblGcode).mockResolvedValueOnce('G21\nM2\n')
      vi.mocked(simulateGcodeViewer).mockResolvedValueOnce(SIM_MESH)

      await renderWithGeneratedToolpath()
      fireEvent.click(screen.getByRole('button', { name: 'Simulate toolpath' }))
      await screen.findByTestId('viewport3d-mock')

      vi.mocked(parseSvg).mockResolvedValueOnce({ paths: [LINE], warnings: [] })
      fireEvent.change(
        screen.getByLabelText('SVG or DXF artwork file') as HTMLInputElement,
        {
          target: {
            files: [new File(['<svg/>'], 'new.svg', { type: 'image/svg+xml' })],
          },
        },
      )

      await waitFor(() => {
        expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()
      })
      expect(useViewportStore.getState().simulationMeshData).toBeNull()
    })
  })
})
