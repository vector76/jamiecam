/**
 * Persistent "recent projects" list, stored in IndexedDB.
 *
 * One object store (`recents`) keyed by `fileName`, so re-loading the
 * same file bumps its timestamp instead of creating a duplicate row.
 * Records are sorted client-side by `savedAt` descending — the recent
 * list is small (capped at MAX_RECENTS) so a full scan + sort is fine.
 */

import { openDB, type DBSchema, type IDBPDatabase } from 'idb'
import type { ProjectState } from './projectFile'

const DB_NAME = 'jamiecam'
const DB_VERSION = 1
const STORE = 'recents'

export const MAX_RECENTS = 10

export interface RecentRecord {
  fileName: string
  savedAt: number
  state: ProjectState
}

interface JamiecamDB extends DBSchema {
  recents: {
    key: string
    value: RecentRecord
  }
}

type JamiecamDBHandle = IDBPDatabase<JamiecamDB>

let dbPromise: Promise<JamiecamDBHandle> | null = null

function getDB(): Promise<JamiecamDBHandle> {
  if (!dbPromise) {
    dbPromise = openDB<JamiecamDB>(DB_NAME, DB_VERSION, {
      upgrade(db) {
        if (!db.objectStoreNames.contains(STORE)) {
          db.createObjectStore(STORE, { keyPath: 'fileName' })
        }
      },
    })
  }
  return dbPromise
}

/** Test-only: drop the cached DB handle so the next call reopens. */
export function __resetRecentsForTests(): void {
  dbPromise = null
}

/** Newest-first list of recent projects, capped at `limit` (default = MAX_RECENTS). */
export async function listRecents(limit = MAX_RECENTS): Promise<RecentRecord[]> {
  const db = await getDB()
  const all = await db.getAll(STORE)
  all.sort((a, b) => b.savedAt - a.savedAt)
  return all.slice(0, limit)
}

/**
 * Insert or update the recent record for the given project state, then
 * prune any older entries beyond MAX_RECENTS. Uses `now()` as savedAt
 * so the entry floats to the top of the list.
 */
export async function upsertRecent(state: ProjectState, now: number = Date.now()): Promise<void> {
  const db = await getDB()
  const record: RecentRecord = { fileName: state.fileName, savedAt: now, state }
  await db.put(STORE, record)
  await prune(db)
}

export async function deleteRecent(fileName: string): Promise<void> {
  const db = await getDB()
  await db.delete(STORE, fileName)
}

async function prune(db: JamiecamDBHandle): Promise<void> {
  const all = await db.getAll(STORE)
  if (all.length <= MAX_RECENTS) return
  all.sort((a, b) => b.savedAt - a.savedAt)
  const surplus = all.slice(MAX_RECENTS)
  const tx = db.transaction(STORE, 'readwrite')
  await Promise.all([...surplus.map((r) => tx.store.delete(r.fileName)), tx.done])
}
