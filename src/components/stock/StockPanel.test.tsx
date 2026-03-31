import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { StockPanel } from './StockPanel'
import { useProjectStore } from '../../store/projectStore'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/stock', () => ({ setStock: vi.fn() }))
vi.mock('../../api/file', () => ({ getProjectSnapshot: vi.fn() }))

const stockApi = await import('../../api/stock')
const fileApi = await import('../../api/file')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const SNAPSHOT_NO_STOCK = {
  stock: null, tools: [], operations: [], wcs: [], projectName: 'test', modelPath: null, modelChecksum: null, modifiedAt: '', projectIsOpen: false, filePath: null, dirty: false,
}
const SNAPSHOT_WITH_STOCK = {
  ...SNAPSHOT_NO_STOCK,
  stock: { type: 'box' as const, origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 80, height: 50 },
}

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  useProjectStore.setState({ snapshot: null, selectedOperationId: null, notifications: [] })
})

// ── Tests ─────────────────────────────────────────────────────────────────────

it('shows "No stock defined" and no Clear button when stock is null', () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_STOCK, selectedOperationId: null, notifications: [] })
  render(<StockPanel />)
  expect(screen.getByText('No stock defined')).toBeInTheDocument()
  expect(screen.queryByRole('button', { name: 'Clear' })).not.toBeInTheDocument()
})

it('shows dimension values and Clear Stock button when stock is defined', () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK, selectedOperationId: null, notifications: [] })
  render(<StockPanel />)
  expect(screen.getByText(/100/)).toBeInTheDocument()
  expect(screen.getByText(/80/)).toBeInTheDocument()
  expect(screen.getByText(/50/)).toBeInTheDocument()
  expect(screen.getByRole('button', { name: 'Clear' })).toBeInTheDocument()
})

it('Set Stock submit calls setStock with form values and refreshes snapshot', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_STOCK, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setStock).mockResolvedValue(undefined)
  vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_WITH_STOCK)

  render(<StockPanel />)

  fireEvent.change(screen.getByLabelText(/Width \(X\)/i), { target: { value: '100' } })
  fireEvent.change(screen.getByLabelText(/Depth \(Y\)/i), { target: { value: '80' } })
  fireEvent.change(screen.getByLabelText(/Height \(Z\)/i), { target: { value: '50' } })
  fireEvent.click(screen.getByRole('button', { name: 'Set Stock' }))

  await waitFor(() => expect(stockApi.setStock).toHaveBeenCalledWith({
    type: 'box',
    origin: { x: 0, y: 0, z: 0 },
    width: 100,
    depth: 80,
    height: 50,
  }))
  expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
  expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT_WITH_STOCK)
})

it('Clear Stock calls setStock(null) and refreshes snapshot', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_WITH_STOCK, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setStock).mockResolvedValue(undefined)
  vi.mocked(fileApi.getProjectSnapshot).mockResolvedValue(SNAPSHOT_NO_STOCK)

  render(<StockPanel />)
  fireEvent.click(screen.getByRole('button', { name: 'Clear' }))

  await waitFor(() => expect(stockApi.setStock).toHaveBeenCalledWith(null))
  expect(fileApi.getProjectSnapshot).toHaveBeenCalled()
})

it('pushes error notification when setStock rejects', async () => {
  useProjectStore.setState({ snapshot: SNAPSHOT_NO_STOCK, selectedOperationId: null, notifications: [] })
  vi.mocked(stockApi.setStock).mockRejectedValue({ kind: 'SaveFailed', message: 'write error' })

  render(<StockPanel />)

  fireEvent.change(screen.getByLabelText(/Width \(X\)/i), { target: { value: '100' } })
  fireEvent.change(screen.getByLabelText(/Depth \(Y\)/i), { target: { value: '80' } })
  fireEvent.change(screen.getByLabelText(/Height \(Z\)/i), { target: { value: '50' } })
  fireEvent.click(screen.getByRole('button', { name: 'Set Stock' }))

  await waitFor(() => expect(useProjectStore.getState().notifications).toContain('write error'))
})
