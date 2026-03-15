/**
 * Tests for MaterialSelectorField.tsx — populates a material dropdown via
 * the feeds API and auto-fills machining parameters when a material is selected.
 *
 * API modules are mocked so tests run in jsdom without a real Tauri context.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MaterialSelectorField } from './MaterialSelectorField'

// ── Module mocks ──────────────────────────────────────────────────────────────

vi.mock('../../api/feeds', () => ({
  listMaterials: vi.fn(),
  lookupFeeds: vi.fn(),
}))

const feedsApi = await import('../../api/feeds')

// ── Fixtures ──────────────────────────────────────────────────────────────────

const MATERIAL_LIST = [{ id: 'carbide-test', displayName: 'Test' }]
const FEED_ENTRY = { spindleSpeedRpm: 8000, feedRateMmpm: 1200 }

// ── Setup ─────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
  vi.mocked(feedsApi.listMaterials).mockResolvedValue(MATERIAL_LIST)
  vi.mocked(feedsApi.lookupFeeds).mockResolvedValue(FEED_ENTRY)
})

// ── Helpers ───────────────────────────────────────────────────────────────────

function renderField(props: Partial<Parameters<typeof MaterialSelectorField>[0]> = {}) {
  const defaults = {
    currentMaterialId: null,
    toolMaterial: 'carbide',
    operationCategory: 'pocket',
    onMaterialChange: vi.fn(),
    onFeedsFetched: vi.fn(),
    onFeedsNotFound: vi.fn(),
  }
  return render(<MaterialSelectorField {...defaults} {...props} />)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

it('selecting a material calls onFeedsFetched with the lookup result', async () => {
  const onFeedsFetched = vi.fn()
  const onMaterialChange = vi.fn()
  renderField({ onFeedsFetched, onMaterialChange })

  // Wait for listMaterials to resolve and populate the dropdown.
  await waitFor(() => screen.getByText('Test'))

  fireEvent.change(screen.getByRole('combobox'), { target: { value: 'carbide-test' } })

  await waitFor(() => {
    expect(feedsApi.lookupFeeds).toHaveBeenCalledWith('carbide-test', 'carbide', 'pocket')
    expect(onFeedsFetched).toHaveBeenCalledWith(FEED_ENTRY)
  })
})

it('selecting a material when toolMaterial is null calls onMaterialChange but NOT lookupFeeds', async () => {
  const onMaterialChange = vi.fn()
  const onFeedsFetched = vi.fn()
  renderField({ toolMaterial: null, onMaterialChange, onFeedsFetched })

  await waitFor(() => screen.getByText('Test'))

  fireEvent.change(screen.getByRole('combobox'), { target: { value: 'carbide-test' } })

  await waitFor(() => {
    expect(onMaterialChange).toHaveBeenCalledWith('carbide-test')
  })
  expect(feedsApi.lookupFeeds).not.toHaveBeenCalled()
  expect(onFeedsFetched).not.toHaveBeenCalled()
})

it('when lookupFeeds rejects with NotFound, onFeedsNotFound is called and a notice appears', async () => {
  vi.mocked(feedsApi.lookupFeeds).mockRejectedValue({ kind: 'NotFound', message: 'not found' })
  const onFeedsNotFound = vi.fn()
  const onFeedsFetched = vi.fn()
  renderField({ onFeedsNotFound, onFeedsFetched })

  await waitFor(() => screen.getByText('Test'))

  fireEvent.change(screen.getByRole('combobox'), { target: { value: 'carbide-test' } })

  await waitFor(() => {
    expect(onFeedsNotFound).toHaveBeenCalled()
    expect(document.querySelector('.material-not-found-notice')).toBeTruthy()
  })
  expect(onFeedsFetched).not.toHaveBeenCalled()
})

it('changing toolMaterial prop from null to a real value while currentMaterialId is set triggers lookup', async () => {
  const onFeedsFetched = vi.fn()
  const { rerender } = render(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial={null}
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => screen.getByRole('combobox'))
  expect(feedsApi.lookupFeeds).not.toHaveBeenCalled()

  rerender(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="carbide"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => {
    expect(feedsApi.lookupFeeds).toHaveBeenCalledWith('carbide-test', 'carbide', 'pocket')
    expect(onFeedsFetched).toHaveBeenCalledWith(FEED_ENTRY)
  })
})

it('changing toolMaterial from one value to another re-triggers lookup; NotFound calls onFeedsNotFound', async () => {
  vi.mocked(feedsApi.lookupFeeds).mockResolvedValue(FEED_ENTRY)
  const onFeedsFetched = vi.fn()
  const onFeedsNotFound = vi.fn()

  const { rerender } = render(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="carbide"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={onFeedsNotFound}
    />,
  )

  await waitFor(() => expect(onFeedsFetched).toHaveBeenCalledTimes(1))

  // Switch to a tool material that has no entry.
  vi.mocked(feedsApi.lookupFeeds).mockRejectedValue({ kind: 'NotFound', message: 'not found' })

  rerender(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="hss"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={onFeedsNotFound}
    />,
  )

  await waitFor(() => {
    expect(feedsApi.lookupFeeds).toHaveBeenCalledWith('carbide-test', 'hss', 'pocket')
    expect(onFeedsNotFound).toHaveBeenCalled()
  })
  expect(onFeedsFetched).toHaveBeenCalledTimes(1) // no additional call
})

it('changing operationCategory while currentMaterialId is set re-triggers lookup', async () => {
  const onFeedsFetched = vi.fn()
  const { rerender } = render(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="carbide"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => expect(feedsApi.lookupFeeds).toHaveBeenCalledWith('carbide-test', 'carbide', 'pocket'))

  rerender(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="carbide"
      operationCategory="finishing"
      onMaterialChange={vi.fn()}
      onFeedsFetched={onFeedsFetched}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => {
    expect(feedsApi.lookupFeeds).toHaveBeenCalledWith('carbide-test', 'carbide', 'finishing')
  })
})

it('selecting a material updates the visible select value', async () => {
  const { rerender } = render(
    <MaterialSelectorField
      currentMaterialId={null}
      toolMaterial="carbide"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={vi.fn()}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => screen.getByText('Test'))

  const select = screen.getByRole('combobox') as HTMLSelectElement
  expect(select.value).toBe('')

  rerender(
    <MaterialSelectorField
      currentMaterialId="carbide-test"
      toolMaterial="carbide"
      operationCategory="pocket"
      onMaterialChange={vi.fn()}
      onFeedsFetched={vi.fn()}
      onFeedsNotFound={vi.fn()}
    />,
  )

  await waitFor(() => {
    expect((screen.getByRole('combobox') as HTMLSelectElement).value).toBe('carbide-test')
  })
})
