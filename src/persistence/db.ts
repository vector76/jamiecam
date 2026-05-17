/**
 * Shared IndexedDB handle for the `jamiecam` database.
 *
 * One DB, multiple object stores — `recents` (project history) and
 * `workingEnv` (machine setups, tools, and the availability matrix). Both
 * modules call `getDB()` here so the version and upgrade callback stay in
 * one place; otherwise the second module to open the DB would race the
 * first into a VersionError.
 */

import { openDB, type IDBPDatabase } from 'idb'

export const DB_NAME = 'jamiecam'
export const DB_VERSION = 2

export const RECENTS_STORE = 'recents'
export const WORKING_ENV_STORE = 'workingEnv'

let dbPromise: Promise<IDBPDatabase> | null = null

export function getDB(): Promise<IDBPDatabase> {
  if (!dbPromise) {
    dbPromise = openDB(DB_NAME, DB_VERSION, {
      upgrade(db, oldVersion) {
        if (oldVersion < 1) {
          db.createObjectStore(RECENTS_STORE, { keyPath: 'fileName' })
        }
        if (oldVersion < 2) {
          // Out-of-line keys ('setups' | 'tools' | 'availability'); each
          // record holds one collection of the WorkingEnvironment aggregate.
          db.createObjectStore(WORKING_ENV_STORE)
        }
      },
    })
  }
  return dbPromise
}

/** Test-only: drop the cached DB handle so the next call reopens. */
export function __resetDBForTests(): void {
  dbPromise = null
}
