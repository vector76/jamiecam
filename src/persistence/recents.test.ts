import { IDBFactory } from 'fake-indexeddb'
import {
  listRecents,
  upsertRecent,
  deleteRecent,
  __resetRecentsForTests,
  MAX_RECENTS,
  type RecentRecord,
} from './recents'
import type { ProjectState } from './projectFile'

function makeState(fileName: string, suffix = ''): ProjectState {
  return {
    fileName,
    mode: 'gcode-viewer',
    payload: {
      gcode: `G0 X0${suffix}\n`,
      sim: {
        stock: { origin: { x: 0, y: 0, z: 0 }, width: 100, depth: 50, height: 10 },
        toolDiameter: 6,
        resolution: 0.5,
      },
    },
  }
}

beforeEach(() => {
  // Fresh in-memory DB per test so they don't see each other's data.
  globalThis.indexedDB = new IDBFactory()
  __resetRecentsForTests()
})

describe('recents', () => {
  it('returns an empty list when nothing has been saved', async () => {
    expect(await listRecents()).toEqual([])
  })

  it('upserts and lists newest-first', async () => {
    await upsertRecent(makeState('a.nc'), 100)
    await upsertRecent(makeState('b.nc'), 200)
    await upsertRecent(makeState('c.nc'), 150)

    const list = await listRecents()
    expect(list.map((r) => r.fileName)).toEqual(['b.nc', 'c.nc', 'a.nc'])
  })

  it('re-saving the same fileName updates the existing record (no duplicate)', async () => {
    await upsertRecent(makeState('a.nc'), 100)
    await upsertRecent(makeState('a.nc', '-updated'), 200)

    const list = await listRecents()
    expect(list).toHaveLength(1)
    expect(list[0].savedAt).toBe(200)
    expect(list[0].state.mode).toBe('gcode-viewer')
    if (list[0].state.mode === 'gcode-viewer') {
      expect(list[0].state.payload.gcode).toBe('G0 X0-updated\n')
    }
  })

  it(`prunes beyond MAX_RECENTS (${MAX_RECENTS})`, async () => {
    for (let i = 0; i < MAX_RECENTS + 5; i++) {
      await upsertRecent(makeState(`file-${i}.nc`), i)
    }
    const list = await listRecents(100)
    expect(list).toHaveLength(MAX_RECENTS)
    // Newest 10 retained — file-4 through file-14, sorted newest-first.
    expect(list[0].fileName).toBe(`file-${MAX_RECENTS + 4}.nc`)
    expect(list[list.length - 1].fileName).toBe('file-5.nc')
  })

  it('listRecents honours a smaller limit', async () => {
    await upsertRecent(makeState('a.nc'), 100)
    await upsertRecent(makeState('b.nc'), 200)
    await upsertRecent(makeState('c.nc'), 150)

    const list = await listRecents(2)
    expect(list.map((r) => r.fileName)).toEqual(['b.nc', 'c.nc'])
  })

  it('deleteRecent removes the named entry', async () => {
    await upsertRecent(makeState('a.nc'), 100)
    await upsertRecent(makeState('b.nc'), 200)
    await deleteRecent('a.nc')

    const list = await listRecents()
    expect(list.map((r) => r.fileName)).toEqual(['b.nc'])
  })

  it('round-trips a full ProjectState', async () => {
    const state = makeState('x.nc')
    if (state.mode === 'gcode-viewer') {
      state.payload.sim.toolDiameter = 12.5
    }
    await upsertRecent(state, 42)

    const [recent] = await listRecents()
    const expected: RecentRecord = { fileName: 'x.nc', savedAt: 42, state }
    expect(recent).toEqual(expected)
  })
})
