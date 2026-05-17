/**
 * Tests for the Web Worker client wrapping the simulation worker.
 *
 * Uses a hand-written MockWorker rather than a real Worker so the tests
 * run synchronously in jsdom without spinning up the wasm module.
 */

import { createSimulationClient } from './simulationWorkerClient'
import type { SimWorkerRequest, SimWorkerResponse } from './simulation.worker'
import type { MeshData, SimulateGcodeViewerParams } from './types'

type MessageListener = (event: MessageEvent<SimWorkerResponse>) => void
type ErrorListener = (event: ErrorEvent) => void

class MockWorker {
  posted: SimWorkerRequest[] = []
  terminated = false
  private messageListeners: MessageListener[] = []
  private errorListeners: ErrorListener[] = []

  postMessage(request: SimWorkerRequest) {
    this.posted.push(request)
  }

  terminate() {
    this.terminated = true
  }

  addEventListener(type: 'message' | 'error', listener: MessageListener | ErrorListener) {
    if (type === 'message') this.messageListeners.push(listener as MessageListener)
    else this.errorListeners.push(listener as ErrorListener)
  }

  // Test helpers ─────────────────────────────────────────────────────
  emitResponse(response: SimWorkerResponse) {
    const event = { data: response } as MessageEvent<SimWorkerResponse>
    for (const l of this.messageListeners) l(event)
  }

  emitError(message: string) {
    const event = { message } as ErrorEvent
    for (const l of this.errorListeners) l(event)
  }
}

const PARAMS: SimulateGcodeViewerParams = {
  stock: { origin: { x: 0, y: 0, z: 0 }, width: 10, depth: 10, height: 5 },
  toolDiameter: 3,
  resolution: 0.5,
}

const MESH: MeshData = { vertices: [0, 0, 0], normals: [0, 0, 1], indices: [], faceGroups: [] }

describe('createSimulationClient', () => {
  it('posts a request with an id and resolves with the matching response', async () => {
    const worker = new MockWorker()
    const client = createSimulationClient(worker as unknown as Worker)

    const promise = client.simulate('G0 X0', PARAMS)
    expect(worker.posted).toHaveLength(1)
    const sent = worker.posted[0]
    expect(sent.content).toBe('G0 X0')
    expect(sent.params).toEqual(PARAMS)

    worker.emitResponse({ id: sent.id, ok: true, mesh: MESH })
    await expect(promise).resolves.toBe(MESH)
  })

  it('rejects with the worker-side AppError when ok is false', async () => {
    const worker = new MockWorker()
    const client = createSimulationClient(worker as unknown as Worker)

    const promise = client.simulate('bad', PARAMS)
    const id = worker.posted[0].id
    worker.emitResponse({
      id,
      ok: false,
      error: { kind: 'InvalidInput', message: 'tool diameter must be positive' },
    })

    await expect(promise).rejects.toEqual({
      kind: 'InvalidInput',
      message: 'tool diameter must be positive',
    })
  })

  it('correlates concurrent requests by id', async () => {
    const worker = new MockWorker()
    const client = createSimulationClient(worker as unknown as Worker)

    const a = client.simulate('a', PARAMS)
    const b = client.simulate('b', PARAMS)
    expect(worker.posted.map((r) => r.id)).toHaveLength(2)
    const [idA, idB] = worker.posted.map((r) => r.id)
    expect(idA).not.toBe(idB)

    const meshB: MeshData = { ...MESH, vertices: [1, 1, 1] }
    // Respond out of order — b first, then a — to verify id matching.
    worker.emitResponse({ id: idB, ok: true, mesh: meshB })
    worker.emitResponse({ id: idA, ok: true, mesh: MESH })

    await expect(a).resolves.toBe(MESH)
    await expect(b).resolves.toBe(meshB)
  })

  it('ignores responses with unknown ids without throwing', () => {
    const worker = new MockWorker()
    createSimulationClient(worker as unknown as Worker)

    expect(() => worker.emitResponse({ id: 999, ok: true, mesh: MESH })).not.toThrow()
  })

  it('rejects all pending requests when the worker emits an error event', async () => {
    const worker = new MockWorker()
    const client = createSimulationClient(worker as unknown as Worker)

    const a = client.simulate('a', PARAMS)
    const b = client.simulate('b', PARAMS)
    worker.emitError('worker crashed')

    await expect(a).rejects.toMatchObject({ kind: 'WorkerError' })
    await expect(b).rejects.toMatchObject({ kind: 'WorkerError' })
  })

  it('dispose() terminates the worker and rejects pending requests', async () => {
    const worker = new MockWorker()
    const client = createSimulationClient(worker as unknown as Worker)

    const pending = client.simulate('x', PARAMS)
    client.dispose()

    expect(worker.terminated).toBe(true)
    await expect(pending).rejects.toMatchObject({ kind: 'Disposed' })
  })
})
