import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { WCSPanel } from './WCSPanel'
import { useProjectStore } from '../../store/projectStore'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/stock', () => ({ setWcs: vi.fn() }))
vi.mock('../../api/file', () => ({ getProjectSnapshot: vi.fn() }))

const stockApi = await import('../../api/stock')
const fileApi = await import('../../api/file')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SNAPSHOT_NO_WCS = {
  stock: null, tools: [], operations: [], wcs: [],
  projectName: 'test', modelPath: null, modelChecksum: null, modifiedAt: '',
}
const SNAPSHOT_WITH_WCS = {
  ...SNAPSHOT_NO_WCS,
  wcs: [{
    id: 'wcs-id-1', name: 'G54',
    origin: { x: 10, y: 20, z: 5 },
    xAxis: { x: 1, y: 0, z: 0 },
    zAxis: { x: 0, y: 0, z: 1 },
  }],
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
})

// ── Tests ─────────────────────────────────────────────────────────────────────

it('shows "No WCS defined" and no Clear button when wcs is empty', () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_WCS, selectedOperationId: null, notifications: [] })
  render(<WCSPanel />)
  expect(screen.getByText('No WCS defined')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
})

it('shows origin values and Clear WCS button when WCS is defined', () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_WITH_WCS, selectedOperationId: null, notifications: [] })
  const { container } = render(<WCSPanel />)
  expect(container).toHaveTextContent('10')
  expect(container).toHaveTextContent('20')
  expect(container).toHaveTextContent('5')
  expect(screen.getByRole('button', { name: 'Clear' })).toBeInTheDocument()
})

it('Set WCS assembles correctly when updating existing WCS', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_WITH_WCS, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setWcs).mockResolvedValue(undefined)
  vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue({ ...SNAPSHOT_WITH_WCS, projectName: 'updated' })

  render(<WCSPanel />)
  fireEvent.click(screen.getByRole('button', { name: 'Set WCS' }))

  await waitFor(() => expect(stockApi.setWcs).toHaveBeenCalledWith([{
    id: 'wcs-id-1', name: 'G54',
    origin: { x: 10, y: 20, z: 5 },
    xAxis: { x: 1, y: 0, z: 0 },
    zAxis: { x: 0, y: 0, z: 1 },
  }]))
  expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
  expect(useProjectStore.getState().snapshot?.projectName).toBe('updated')
})

it('Set WCS assembles correctly when creating new WCS (no existing WCS)', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_WCS, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setWcs).mockResolvedValue(undefined)
  vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_WCS)

  render(<WCSPanel />)
  fireEvent.click(screen.getByRole('button', { name: 'Set WCS' }))

  await waitFor(() => expect(stockApi.setWcs).toHaveBeenCalled())
  const [[wcsList]] = vi.mocked(stockApi.setWcs).mock.calls
  const payload = wcsList[0]
  expect(payload.name).toBe('G54')
  expect(payload.xAxis).toEqual({ x: 1, y: 0, z: 0 })
  expect(payload.zAxis).toEqual({ x: 0, y: 0, z: 1 })
  expect(typeof payload.id).toBe('string')
  expect(payload.id.length).toBeGreaterThan(0)
})

it('Clear WCS calls setWcs([]) and refreshes snapshot', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_WITH_WCS, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setWcs).mockResolvedValue(undefined)
  vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_NO_WCS)

  render(<WCSPanel />)
  fireEvent.click(screen.getByRole('button', { name: 'Clear' }))

  await waitFor(() => expect(stockApi.setWcs).toHaveBeenCalledWith([]))
  expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
})

it('pushes error notification when setWcs rejects', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_WCS, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setWcs).mockRejectedValue({ kind: 'SaveFailed', message: 'write error' })

  render(<WCSPanel />)
  fireEvent.click(screen.getByRole('button', { name: 'Set WCS' }))

  await waitFor(() => expect(useProjectStore.getState().notifications).toContain('write error'))
})
