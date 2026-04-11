import { renderHook, act } from '@testing-library/react'
import { useGlobalToolStore, useGlobalTools, refreshProjectSnapshot } from './globalToolStore'
import { useProjectStore } from './projectStore'
import type { Tool, ProjectSnapshot } from '../api/types'

// ── Mocks ────────────────────────────────────────────────────────────────────

vi.mock('../api/globalTools', () => ({
  listGlobalTools: vi.fn(),
}))

vi.mock('../api/file', () => ({
  getProjectSnapshot: vi.fn(),
}))

import { listGlobalTools } from '../api/globalTools'
import { getProjectSnapshot } from '../api/file'

const TOOL_A: Tool = {
  id: 'aaa-111',
  name: '10mm Flat Endmill',
  type: 'flat_endmill',
  material: 'carbide',
  diameter: 10,
  fluteCount: 4,
  cuttingLength: 25,
  shankDiameter: 10,
  overallLength: 75,
}

const TOOL_B: Tool = {
  id: 'bbb-222',
  name: '6mm Ball Nose',
  type: 'ball_nose',
  material: 'hss',
  diameter: 6,
  fluteCount: 2,
  cuttingLength: 18,
  shankDiameter: 6,
  overallLength: 50,
}

const SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test',
  modifiedAt: '',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
  projectIsOpen: false,
  filePath: null,
  dirty: false,
  mode: '3d',
  safeHeight: null,
  artworkOrigin: [0, 0] as [number, number],
}

// ── Setup ────────────────────────────────────────────────────────────────────

beforeEach(() => {
  useGlobalToolStore.setState({ globalTools: [] })
  useProjectStore.setState({ snapshot: null })
  vi.clearAllMocks()
})

// ── State transitions ────────────────────────────────────────────────────────

describe('globalToolStore — state transitions', () => {
  it('starts with an empty globalTools array', () => {
    expect(useGlobalToolStore.getState().globalTools).toEqual([])
  })

  it('setGlobalTools stores the provided tools', () => {
    useGlobalToolStore.getState().setGlobalTools([TOOL_A, TOOL_B])
    expect(useGlobalToolStore.getState().globalTools).toEqual([TOOL_A, TOOL_B])
  })

  it('setGlobalTools replaces the previous list entirely', () => {
    useGlobalToolStore.getState().setGlobalTools([TOOL_A])
    useGlobalToolStore.getState().setGlobalTools([TOOL_B])
    expect(useGlobalToolStore.getState().globalTools).toEqual([TOOL_B])
  })

  it('setGlobalTools with empty array clears tools', () => {
    useGlobalToolStore.getState().setGlobalTools([TOOL_A])
    useGlobalToolStore.getState().setGlobalTools([])
    expect(useGlobalToolStore.getState().globalTools).toEqual([])
  })
})

// ── useGlobalTools selector ──────────────────────────────────────────────────

describe('globalToolStore — useGlobalTools selector', () => {
  it('returns empty array when no tools are set', () => {
    const { result } = renderHook(() => useGlobalTools())
    expect(result.current).toEqual([])
  })

  it('returns tools when tools are set', () => {
    useGlobalToolStore.setState({ globalTools: [TOOL_A] })
    const { result } = renderHook(() => useGlobalTools())
    expect(result.current).toEqual([TOOL_A])
  })

  it('updates when store changes', () => {
    const { result } = renderHook(() => useGlobalTools())
    expect(result.current).toEqual([])

    act(() => {
      useGlobalToolStore.getState().setGlobalTools([TOOL_A, TOOL_B])
    })
    expect(result.current).toHaveLength(2)
    expect(result.current[0].name).toBe('10mm Flat Endmill')
  })

  it('returns stable reference for default empty array', () => {
    const { result, rerender } = renderHook(() => useGlobalTools())
    const first = result.current
    rerender()
    expect(result.current).toBe(first)
  })
})

// ── refreshGlobalTools ───────────────────────────────────────────────────────

describe('globalToolStore — refreshGlobalTools', () => {
  it('fetches tools from backend and updates store', async () => {
    vi.mocked(listGlobalTools).mockResolvedValue([TOOL_A, TOOL_B])

    await useGlobalToolStore.getState().refreshGlobalTools()

    expect(listGlobalTools).toHaveBeenCalledOnce()
    expect(useGlobalToolStore.getState().globalTools).toEqual([TOOL_A, TOOL_B])
  })

  it('replaces existing tools on refresh', async () => {
    useGlobalToolStore.getState().setGlobalTools([TOOL_A])
    vi.mocked(listGlobalTools).mockResolvedValue([TOOL_B])

    await useGlobalToolStore.getState().refreshGlobalTools()

    expect(useGlobalToolStore.getState().globalTools).toEqual([TOOL_B])
  })

  it('propagates errors from the backend', async () => {
    vi.mocked(listGlobalTools).mockRejectedValue({ kind: 'IoError', message: 'disk fail' })

    await expect(useGlobalToolStore.getState().refreshGlobalTools()).rejects.toEqual({
      kind: 'IoError',
      message: 'disk fail',
    })
  })
})

// ── refreshProjectSnapshot ───────────────────────────────────────────────────

describe('refreshProjectSnapshot', () => {
  it('fetches snapshot from backend and sets it in project store', async () => {
    vi.mocked(getProjectSnapshot).mockResolvedValue(SNAPSHOT)

    await refreshProjectSnapshot()

    expect(getProjectSnapshot).toHaveBeenCalledOnce()
    expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT)
  })

  it('replaces existing snapshot', async () => {
    useProjectStore.getState().setSnapshot({ ...SNAPSHOT, projectName: 'Old' })
    vi.mocked(getProjectSnapshot).mockResolvedValue({ ...SNAPSHOT, projectName: 'New' })

    await refreshProjectSnapshot()

    expect(useProjectStore.getState().snapshot?.projectName).toBe('New')
  })

  it('propagates errors from the backend', async () => {
    vi.mocked(getProjectSnapshot).mockRejectedValue({ kind: 'Unknown', message: 'fail' })

    await expect(refreshProjectSnapshot()).rejects.toEqual({
      kind: 'Unknown',
      message: 'fail',
    })
  })
})
