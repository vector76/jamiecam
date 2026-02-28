/**
 * Tests for GCodePreviewPanel.tsx — G-code preview, post-processor selector,
 * and export flow.
 *
 * The toolpath API and tauri-plugin-dialog are mocked so tests run in jsdom
 * without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { GCodePreviewPanel } from './GCodePreviewPanel'
import { useProjectStore } from '../../store/projectStore'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/toolpath', () => ({
  listPostProcessors: vi.fn(),
  getGcodePreview: vi.fn(),
  exportGcode: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: vi.fn(),
}))

const toolpathApi = await import('../../api/toolpath')
const dialogApi = await import('@tauri-apps/plugin-dialog')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const OP_ID = 'cccccccc-0000-0000-0000-000000000001'

const PP_LIST = [
  { id: 'linuxcnc', name: 'LinuxCNC', description: '' },
  { id: 'fanuc-0i', name: 'Fanuc 0i', description: '' },
]

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
  vi.mocked(toolpathApi.listPostProcessors).mockResolvedValue(PP_LIST)
  vi.mocked(toolpathApi.getGcodePreview).mockResolvedValue('')
  vi.mocked(toolpathApi.exportGcode).mockResolvedValue(undefined)
})

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('GCodePreviewPanel', () => {
  it('renders placeholder when no operation selected', async () => {
    useProjectStore.setState({ selectedOperationId: null, notifications: [] })
    render(<GCodePreviewPanel />)
    expect(screen.getByText('Select an operation to preview G-code.')).toBeInTheDocument()
  })

  it('renders placeholder when getGcodePreview rejects with NotFound', async () => {
    vi.mocked(toolpathApi.getGcodePreview).mockRejectedValue({ kind: 'NotFound', message: 'no toolpath' })
    useProjectStore.setState({ selectedOperationId: OP_ID, notifications: [] })
    render(<GCodePreviewPanel />)
    await waitFor(() => {
      expect(screen.getByText('No toolpath computed for this operation.')).toBeInTheDocument()
    })
  })

  it('renders gcode text when preview available', async () => {
    vi.mocked(toolpathApi.listPostProcessors).mockResolvedValue([{ id: 'linuxcnc', name: 'LinuxCNC', description: '' }])
    vi.mocked(toolpathApi.getGcodePreview).mockResolvedValue('G00 X0 Y0\nG01 X10')
    useProjectStore.setState({ selectedOperationId: OP_ID, notifications: [] })
    render(<GCodePreviewPanel />)
    await waitFor(() => {
      expect(screen.getByText('G00 X0 Y0\nG01 X10', { normalizer: (s) => s })).toBeInTheDocument()
    })
  })

  it('calls exportGcode when Export button clicked', async () => {
    vi.mocked(dialogApi.save).mockResolvedValue('/tmp/output.nc')
    vi.mocked(toolpathApi.getGcodePreview).mockResolvedValue('G00 X0 Y0')
    vi.mocked(toolpathApi.exportGcode).mockResolvedValue(undefined)
    useProjectStore.setState({ selectedOperationId: OP_ID, notifications: [] })
    render(<GCodePreviewPanel />)

    // Wait for gcode to load so Export button is enabled
    await waitFor(() => {
      expect(screen.getByRole('button', { name: 'Export…' })).not.toBeDisabled()
    })

    fireEvent.click(screen.getByRole('button', { name: 'Export…' }))

    await waitFor(() => {
      expect(toolpathApi.exportGcode).toHaveBeenCalledWith(
        expect.objectContaining({ outputPath: '/tmp/output.nc' })
      )
    })
  })

  it('post-processor selector populated from listPostProcessors', async () => {
    useProjectStore.setState({ selectedOperationId: OP_ID, notifications: [] })
    render(<GCodePreviewPanel />)
    await waitFor(() => {
      expect(screen.getByRole('option', { name: 'LinuxCNC' })).toBeInTheDocument()
      expect(screen.getByRole('option', { name: 'Fanuc 0i' })).toBeInTheDocument()
    })
  })
})
