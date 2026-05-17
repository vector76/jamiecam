import { IDBFactory } from 'fake-indexeddb'
import { openDB } from 'idb'
import { __resetDBForTests, getDB } from './db'

beforeEach(() => {
  globalThis.indexedDB = new IDBFactory()
  __resetDBForTests()
})

describe('jamiecam DB v1 → v2 migration', () => {
  it('preserves data in the recents store when upgrading to v2', async () => {
    // Seed an existing v1 DB the same way the pre-workingEnv shipped code did.
    const v1 = await openDB('jamiecam', 1, {
      upgrade(db) {
        db.createObjectStore('recents', { keyPath: 'fileName' })
      },
    })
    await v1.put('recents', { fileName: 'a.nc', savedAt: 100, state: { gcode: 'G0\n' } })
    v1.close()

    // Now open via the shared module — should upgrade to v2 and add workingEnv
    // while keeping the existing recents row intact.
    const db = await getDB()
    expect([...db.objectStoreNames].sort()).toEqual(['recents', 'workingEnv'])

    const kept = await db.get('recents', 'a.nc')
    expect(kept).toEqual({ fileName: 'a.nc', savedAt: 100, state: { gcode: 'G0\n' } })
  })
})
