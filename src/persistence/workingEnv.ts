/**
 * Persistent working environment — machine setups, tools, and the
 * compatibility matrix between them — stored in IndexedDB.
 *
 * Per `docs/phase-4-design.md` §6 the working environment lives outside
 * `.jcam` project files because it describes the user's CNC hardware
 * rather than any particular project.
 *
 * Storage layout: one shared object store (`workingEnv`) in the `jamiecam`
 * database, with one record per collection of the aggregate, keyed by
 * `'setups' | 'tools' | 'availability'`. Splitting per collection means a
 * single `saveWorkingEnv` writes three small records in one transaction
 * (no `JSON.stringify` round-trip), and a future "edit just the tools UI"
 * flow can write the tools record without touching the others.
 *
 * No import/export to disk is wired up yet — IndexedDB is the only home
 * for now. File-based export lands when the editing UI needs it.
 */

import type { IDBPDatabase } from 'idb'
import { getDB, WORKING_ENV_STORE as STORE } from './db'
import type {
  AvailabilityMatrix,
  AvailabilityPair,
  MachineSetup,
  Tool,
  WorkingEnvironment,
} from '../api/types'

const KEY_SETUPS = 'setups'
const KEY_TOOLS = 'tools'
const KEY_AVAILABILITY = 'availability'

/**
 * Load the working environment. Missing collections default to empty
 * arrays so a partially-written DB still produces a valid aggregate.
 */
export async function loadWorkingEnv(): Promise<WorkingEnvironment> {
  const db = await getDB()
  const tx = db.transaction(STORE, 'readonly')
  const [setups, tools, availability] = (await Promise.all([
    tx.store.get(KEY_SETUPS),
    tx.store.get(KEY_TOOLS),
    tx.store.get(KEY_AVAILABILITY),
  ])) as [MachineSetup[] | undefined, Tool[] | undefined, AvailabilityMatrix | undefined]
  await tx.done
  return {
    setups: setups ?? [],
    tools: tools ?? [],
    availability: availability ?? [],
  }
}

/** Write all three collections atomically in a single readwrite transaction. */
export async function saveWorkingEnv(env: WorkingEnvironment): Promise<void> {
  const db = await getDB()
  await writeAll(db, env)
}

/**
 * If no working environment has been saved yet, seed a minimal one — one
 * placeholder setup, one placeholder tool, and one availability entry
 * linking them — so first-run UI has something to render and edit.
 * Returns the current environment either way.
 */
export async function seedIfEmpty(
  newId: () => string = defaultNewId,
): Promise<WorkingEnvironment> {
  const current = await loadWorkingEnv()
  if (!isEmpty(current)) return current

  const setup: MachineSetup = {
    id: newId(),
    name: 'Default Machine',
    workspace: {
      origin: { x: 0, y: 0, z: 0 },
      width: 300,
      depth: 200,
      height: 80,
    },
    kinematics: '3-axis-router',
    postProcessor: 'grbl-1.1',
    safety: { safeZ: 5, rapidFeedRate: 3000 },
  }
  const tool: Tool = {
    id: newId(),
    name: '1/8" Flat End Mill',
    diameter: 3.175,
    fluteCount: 2,
    length: 38,
    material: 'carbide',
    recommended: { spindleRpm: 18000, feedRate: 800, plungeRate: 200 },
  }
  const pair: AvailabilityPair = { setupId: setup.id, toolId: tool.id }

  const seeded: WorkingEnvironment = {
    setups: [setup],
    tools: [tool],
    availability: [pair],
  }
  await saveWorkingEnv(seeded)
  return seeded
}

function isEmpty(env: WorkingEnvironment): boolean {
  return env.setups.length === 0 && env.tools.length === 0 && env.availability.length === 0
}

async function writeAll(db: IDBPDatabase, env: WorkingEnvironment): Promise<void> {
  const tx = db.transaction(STORE, 'readwrite')
  await Promise.all([
    tx.store.put(env.setups, KEY_SETUPS),
    tx.store.put(env.tools, KEY_TOOLS),
    tx.store.put(env.availability, KEY_AVAILABILITY),
    tx.done,
  ])
}

function defaultNewId(): string {
  // `crypto.randomUUID` is available in modern browsers and Node ≥ 14.17,
  // including the jsdom test environment.
  return crypto.randomUUID()
}
