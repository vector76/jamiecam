/**
 * Tests for Mode2ProfileMode — the Phase 4 (2-D Profile Cuts) shell.
 *
 * The wasm bridge, the Mode 2 parser API, and the Canvas2D viewport are
 * mocked so we can verify layout, file picker plumbing, parser-error
 * surfacing, and the Paths selection list without booting WebAssembly
 * or a real canvas.
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Mode2ProfileMode } from './Mode2ProfileMode'
import { useViewport2DStore } from '../../store/viewport2dStore'
import type {
  AppError,
  ParseDxfResult,
  ParseSvgResult,
  Polyline,
} from '../../api/types'

vi.mock('../../api/gcodeViewer', () => ({
  prewarmWasm: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../../api/mode2', () => ({
  parseSvg: vi.fn(),
  parseDxf: vi.fn(),
}))

vi.mock('../../viewport2d/Canvas2DViewport', () => ({
  Canvas2DViewport: () => <div data-testid="canvas2d-mock" />,
}))

import { prewarmWasm } from '../../api/gcodeViewer'
import { parseDxf, parseSvg } from '../../api/mode2'

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
})

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
})
