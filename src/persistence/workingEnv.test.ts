import { IDBFactory } from 'fake-indexeddb'
import { __resetDBForTests } from './db'
import { loadWorkingEnv, saveWorkingEnv, seedIfEmpty } from './workingEnv'
import type { MachineSetup, Tool, WorkingEnvironment } from '../api/types'

function makeSetup(id: string): MachineSetup {
  return {
    id,
    name: `Setup ${id}`,
    workspace: { origin: { x: 0, y: 0, z: 0 }, width: 400, depth: 300, height: 100 },
    kinematics: '3-axis-router',
    postProcessor: 'grbl-1.1',
    safety: { safeZ: 5, rapidFeedRate: 3000 },
  }
}

function makeTool(id: string): Tool {
  return {
    id,
    name: `Tool ${id}`,
    diameter: 6,
    fluteCount: 2,
    length: 40,
    material: 'carbide',
    recommended: { spindleRpm: 16000, feedRate: 1000, plungeRate: 250 },
  }
}

beforeEach(() => {
  // Fresh in-memory DB per test so they don't see each other's data.
  globalThis.indexedDB = new IDBFactory()
  __resetDBForTests()
})

describe('workingEnv persistence', () => {
  it('loadWorkingEnv returns empty collections when nothing is saved', async () => {
    const env = await loadWorkingEnv()
    expect(env).toEqual<WorkingEnvironment>({ setups: [], tools: [], availability: [] })
  })

  it('round-trips a populated WorkingEnvironment through save+load', async () => {
    const env: WorkingEnvironment = {
      setups: [makeSetup('s1'), makeSetup('s2')],
      tools: [makeTool('t1'), makeTool('t2')],
      availability: [
        { setupId: 's1', toolId: 't1' },
        { setupId: 's2', toolId: 't2' },
      ],
    }
    await saveWorkingEnv(env)

    const loaded = await loadWorkingEnv()
    expect(loaded).toEqual(env)
  })

  it('saveWorkingEnv overwrites the previous record', async () => {
    await saveWorkingEnv({
      setups: [makeSetup('s1')],
      tools: [makeTool('t1')],
      availability: [{ setupId: 's1', toolId: 't1' }],
    })
    await saveWorkingEnv({ setups: [], tools: [], availability: [] })

    const loaded = await loadWorkingEnv()
    expect(loaded).toEqual<WorkingEnvironment>({ setups: [], tools: [], availability: [] })
  })

  it('seedIfEmpty populates one setup, one tool, and a linking availability pair on first run', async () => {
    let counter = 0
    const newId = () => `id-${++counter}`

    const env = await seedIfEmpty(newId)

    expect(env.setups).toHaveLength(1)
    expect(env.tools).toHaveLength(1)
    expect(env.availability).toEqual([{ setupId: env.setups[0].id, toolId: env.tools[0].id }])

    // The seed must have been persisted, not just returned.
    expect(await loadWorkingEnv()).toEqual(env)
  })

  it('seedIfEmpty is a no-op when any data is already present', async () => {
    const preExisting: WorkingEnvironment = {
      setups: [makeSetup('only')],
      tools: [],
      availability: [],
    }
    await saveWorkingEnv(preExisting)

    const env = await seedIfEmpty(() => 'should-not-be-used')
    expect(env).toEqual(preExisting)
    expect(await loadWorkingEnv()).toEqual(preExisting)
  })

  it('default seedIfEmpty mints non-empty distinct ids', async () => {
    const env = await seedIfEmpty()
    expect(typeof env.setups[0].id).toBe('string')
    expect(env.setups[0].id.length).toBeGreaterThan(0)
    expect(typeof env.tools[0].id).toBe('string')
    expect(env.tools[0].id.length).toBeGreaterThan(0)
    expect(env.setups[0].id).not.toBe(env.tools[0].id)
  })
})
