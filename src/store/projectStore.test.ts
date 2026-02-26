import { renderHook, act } from '@testing-library/react'
import {
  useProjectStore,
  useModelPath,
  useModelChecksum,
  useOperations,
  useTools,
  useStock,
} from './projectStore'
import type { OperationSummary, ProjectSnapshot, StockDefinition, ToolSummary } from '../api/types'

const SNAPSHOT: ProjectSnapshot = {
  modelPath: '/home/user/part.step',
  modelChecksum: 'abc123def456',
  projectName: 'Test Project',
  modifiedAt: '2026-01-01T00:00:00Z',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
}

beforeEach(() => {
  // Reset store to initial state between tests.
  useProjectStore.setState({ snapshot: null })
})

describe('projectStore — state transitions', () => {
  it('starts with a null snapshot', () => {
    expect(useProjectStore.getState().snapshot).toBeNull()
  })

  it('setSnapshot stores the provided snapshot', () => {
    useProjectStore.getState().setSnapshot(SNAPSHOT)
    expect(useProjectStore.getState().snapshot).toEqual(SNAPSHOT)
  })

  it('setSnapshot(null) clears the snapshot', () => {
    useProjectStore.getState().setSnapshot(SNAPSHOT)
    useProjectStore.getState().setSnapshot(null)
    expect(useProjectStore.getState().snapshot).toBeNull()
  })

  it('setSnapshot replaces the previous snapshot entirely', () => {
    const first: ProjectSnapshot = { ...SNAPSHOT, projectName: 'First' }
    const second: ProjectSnapshot = { ...SNAPSHOT, projectName: 'Second' }
    useProjectStore.getState().setSnapshot(first)
    useProjectStore.getState().setSnapshot(second)
    expect(useProjectStore.getState().snapshot?.projectName).toBe('Second')
  })
})

describe('projectStore — useModelPath selector', () => {
  it('returns null when snapshot is null', () => {
    const { result } = renderHook(() => useModelPath())
    expect(result.current).toBeNull()
  })

  it('returns modelPath when snapshot is set', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    const { result } = renderHook(() => useModelPath())
    expect(result.current).toBe('/home/user/part.step')
  })

  it('returns null when snapshot has null modelPath', () => {
    useProjectStore.setState({
      snapshot: { ...SNAPSHOT, modelPath: null },
    })
    const { result } = renderHook(() => useModelPath())
    expect(result.current).toBeNull()
  })

  it('updates when snapshot changes', () => {
    const { result } = renderHook(() => useModelPath())
    expect(result.current).toBeNull()

    act(() => {
      useProjectStore.getState().setSnapshot(SNAPSHOT)
    })
    expect(result.current).toBe('/home/user/part.step')

    act(() => {
      useProjectStore.getState().setSnapshot(null)
    })
    expect(result.current).toBeNull()
  })
})

describe('projectStore — useModelChecksum selector', () => {
  it('returns null when snapshot is null', () => {
    const { result } = renderHook(() => useModelChecksum())
    expect(result.current).toBeNull()
  })

  it('returns modelChecksum when snapshot is set', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    const { result } = renderHook(() => useModelChecksum())
    expect(result.current).toBe('abc123def456')
  })

  it('returns null when snapshot has null modelChecksum', () => {
    useProjectStore.setState({
      snapshot: { ...SNAPSHOT, modelChecksum: null },
    })
    const { result } = renderHook(() => useModelChecksum())
    expect(result.current).toBeNull()
  })

  it('updates when snapshot changes', () => {
    const { result } = renderHook(() => useModelChecksum())
    expect(result.current).toBeNull()

    act(() => {
      useProjectStore.getState().setSnapshot(SNAPSHOT)
    })
    expect(result.current).toBe('abc123def456')
  })
})

describe('projectStore — useOperations selector', () => {
  it('returns empty array when snapshot is null', () => {
    const { result } = renderHook(() => useOperations())
    expect(result.current).toEqual([])
  })

  it('returns empty array when operations is empty', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    const { result } = renderHook(() => useOperations())
    expect(result.current).toEqual([])
  })

  it('returns operations when snapshot has operations', () => {
    const op: OperationSummary = {
      id: 'op-1',
      name: 'Outer Profile',
      operationType: 'profile',
      enabled: true,
      needsRecalculate: true,
    }
    useProjectStore.setState({ snapshot: { ...SNAPSHOT, operations: [op] } })
    const { result } = renderHook(() => useOperations())
    expect(result.current).toEqual([op])
  })

  it('updates when snapshot changes', () => {
    const { result } = renderHook(() => useOperations())
    expect(result.current).toEqual([])

    const op: OperationSummary = {
      id: 'op-1',
      name: 'Drill Holes',
      operationType: 'drill',
      enabled: false,
      needsRecalculate: true,
    }
    act(() => {
      useProjectStore.getState().setSnapshot({ ...SNAPSHOT, operations: [op] })
    })
    expect(result.current).toHaveLength(1)
    expect(result.current[0].name).toBe('Drill Holes')
  })
})

describe('projectStore — useTools selector', () => {
  it('returns empty array when snapshot is null', () => {
    const { result } = renderHook(() => useTools())
    expect(result.current).toEqual([])
  })

  it('returns empty array when tools is empty', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    const { result } = renderHook(() => useTools())
    expect(result.current).toEqual([])
  })

  it('returns tools when snapshot has tools', () => {
    const tool: ToolSummary = { id: 'tool-1', name: '10mm Endmill', toolType: 'flat_endmill' }
    useProjectStore.setState({ snapshot: { ...SNAPSHOT, tools: [tool] } })
    const { result } = renderHook(() => useTools())
    expect(result.current).toEqual([tool])
  })
})

describe('projectStore — useStock selector', () => {
  it('returns null when snapshot is null', () => {
    const { result } = renderHook(() => useStock())
    expect(result.current).toBeNull()
  })

  it('returns null when stock is not set', () => {
    useProjectStore.setState({ snapshot: SNAPSHOT })
    const { result } = renderHook(() => useStock())
    expect(result.current).toBeNull()
  })

  it('returns stock when snapshot has stock', () => {
    const stock: StockDefinition = {
      type: 'box',
      origin: { x: 0, y: 0, z: 0 },
      width: 100,
      depth: 80,
      height: 20,
    }
    useProjectStore.setState({ snapshot: { ...SNAPSHOT, stock } })
    const { result } = renderHook(() => useStock())
    expect(result.current).toEqual(stock)
  })
})
