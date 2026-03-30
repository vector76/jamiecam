import { openToolEditor } from './window'

// ── Mocks ────────────────────────────────────────────────────────────────────

const mockFocus = vi.fn()
const mockOnce = vi.fn()

vi.mock('@tauri-apps/api/webviewWindow', () => {
  const MockWebviewWindow = vi.fn(() => ({
    once: vi.fn(),
  }))
  ;(MockWebviewWindow as unknown as Record<string, unknown>).getByLabel = vi.fn()
  return { WebviewWindow: MockWebviewWindow }
})

import { WebviewWindow } from '@tauri-apps/api/webviewWindow'

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  vi.clearAllMocks()
})

// ── Tests ────────────────────────────────────────────────────────────────────

describe('openToolEditor', () => {
  it('creates a new WebviewWindow when none exists', async () => {
    vi.mocked(WebviewWindow.getByLabel).mockResolvedValue(null)

    await openToolEditor()

    expect(WebviewWindow.getByLabel).toHaveBeenCalledWith('tool-editor')
    expect(WebviewWindow).toHaveBeenCalledWith('tool-editor', {
      url: '/',
      title: 'Tool Editor',
      width: 900,
      height: 650,
    })
  })

  it('focuses the existing window instead of creating a new one', async () => {
    vi.mocked(WebviewWindow.getByLabel).mockResolvedValue({
      setFocus: mockFocus,
    } as unknown as WebviewWindow)

    await openToolEditor()

    expect(WebviewWindow.getByLabel).toHaveBeenCalledWith('tool-editor')
    expect(WebviewWindow).not.toHaveBeenCalledWith('tool-editor', expect.anything())
    expect(mockFocus).toHaveBeenCalled()
  })

  it('registers a tauri://error listener on new windows', async () => {
    vi.mocked(WebviewWindow.getByLabel).mockResolvedValue(null)
    const mockInstance = { once: mockOnce }
    vi.mocked(WebviewWindow).mockReturnValue(mockInstance as unknown as WebviewWindow)

    await openToolEditor()

    expect(mockOnce).toHaveBeenCalledWith('tauri://error', expect.any(Function))
  })
})
