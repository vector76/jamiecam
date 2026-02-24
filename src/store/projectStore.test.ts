import { renderHook, act } from '@testing-library/react'
import {
  useProjectStore,
  useModelPath,
  useModelChecksum,
} from './projectStore'
import type { ProjectSnapshot } from '../api/types'

const SNAPSHOT: ProjectSnapshot = {
  modelPath: '/home/user/part.step',
  modelChecksum: 'abc123def456',
  projectName: 'Test Project',
  modifiedAt: '2026-01-01T00:00:00Z',
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
