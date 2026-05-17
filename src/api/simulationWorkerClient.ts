/**
 * Promise-based client over the simulation Web Worker.
 *
 * Pure function over a `Worker` instance so it can be unit-tested with a
 * mock worker. `gcodeViewer.ts` wraps this in a lazy singleton backed by
 * the real `simulation.worker.ts`.
 *
 * Each `simulate()` call gets a monotonically-increasing id; the worker
 * echoes it back so concurrent requests resolve to the right promise.
 */

import type { AppError, MeshData, SimulateGcodeViewerParams } from './types'
import type { SimWorkerRequest, SimWorkerResponse } from './simulation.worker'

export interface SimulationClient {
  simulate(content: string, params: SimulateGcodeViewerParams): Promise<MeshData>
  dispose(): void
}

interface PendingHandlers {
  resolve: (mesh: MeshData) => void
  reject: (err: AppError) => void
}

export function createSimulationClient(worker: Worker): SimulationClient {
  let nextId = 0
  const pending = new Map<number, PendingHandlers>()

  function rejectAll(err: AppError) {
    for (const handlers of pending.values()) handlers.reject(err)
    pending.clear()
  }

  worker.addEventListener('message', (event: MessageEvent<SimWorkerResponse>) => {
    const response = event.data
    const handlers = pending.get(response.id)
    if (!handlers) return
    pending.delete(response.id)
    if (response.ok) handlers.resolve(response.mesh)
    else handlers.reject(response.error)
  })

  worker.addEventListener('error', (event: ErrorEvent) => {
    rejectAll({
      kind: 'WorkerError',
      message: event.message || 'simulation worker crashed',
    })
  })

  return {
    simulate(content, params) {
      return new Promise<MeshData>((resolve, reject) => {
        const id = nextId++
        pending.set(id, { resolve, reject })
        const request: SimWorkerRequest = { id, content, params }
        worker.postMessage(request)
      })
    },
    dispose() {
      worker.terminate()
      rejectAll({ kind: 'Disposed', message: 'simulation worker disposed' })
    },
  }
}
