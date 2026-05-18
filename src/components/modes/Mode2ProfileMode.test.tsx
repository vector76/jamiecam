/**
 * Tests for Mode2ProfileMode — the Phase 4 (2-D Profile Cuts) shell.
 *
 * The wasm bridge and the Canvas2D viewport are mocked so we can verify
 * layout without booting WebAssembly or a real canvas. Tests focus on
 * structure (sidebar sections present) and the engine-init lifecycle
 * mirrored from Mode 1.
 */

import { render, screen, waitFor } from '@testing-library/react'
import { Mode2ProfileMode } from './Mode2ProfileMode'

vi.mock('../../api/gcodeViewer', () => ({
  prewarmWasm: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../../viewport2d/Canvas2DViewport', () => ({
  Canvas2DViewport: () => <div data-testid="canvas2d-mock" />,
}))

import { prewarmWasm } from '../../api/gcodeViewer'

beforeEach(() => {
  vi.clearAllMocks()
})

describe('Mode2ProfileMode', () => {
  it('renders the Canvas2D workspace alongside the sidebar sections', async () => {
    render(<Mode2ProfileMode />)

    expect(screen.getByTestId('canvas2d-mock')).toBeInTheDocument()

    for (const title of ['File', 'Paths', 'Operation', 'Generate', 'Simulate', 'Export']) {
      expect(screen.getByRole('button', { name: title })).toBeInTheDocument()
    }

    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
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
    await waitFor(() => {
      expect(screen.queryByText('Initializing engine…')).not.toBeInTheDocument()
    })
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
})
