/**
 * OperationEditorForm — inline form for editing a single machining operation.
 *
 * Receives an operationId prop. When null, renders an empty state. When set,
 * fetches the full Operation via listOperations(), renders a form, and calls
 * editOperation on each field change (on blur for number inputs, on change for
 * the tool select). After each save the project snapshot is refreshed.
 *
 * Pocket and profile operations have parameter forms; drill shows a "coming
 * soon" placeholder.
 */

import { useEffect, useState } from 'react'
import { useTools, useProjectStore } from '../../store/projectStore'
import { editOperation, listOperations } from '../../api/operations'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { Operation, OperationInput, PocketParams, ProfileParams, DrillParams } from '../../api/types'

interface Props {
  operationId: string | null
}

export function OperationEditorForm({ operationId }: Props) {
  const tools = useTools()
  const setSnapshot = useProjectStore((s) => s.setSnapshot)
  const pushNotification = useProjectStore((s) => s.pushNotification)
  const [operation, setOperation] = useState<Operation | null>(null)

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  useEffect(() => {
    if (!operationId) { setOperation(null); return }
    listOperations()
      .then((ops) => setOperation(ops.find((o) => o.id === operationId) ?? null))
      .catch(handleError)
  }, [operationId])

  if (!operationId || !operation) {
    return <div style={{ padding: '0.5rem', color: '#888' }}>Select an operation to edit</div>
  }

  async function save(patch: Partial<OperationInput>) {
    if (!operation) return
    try {
      const input: OperationInput = {
        name: operation.name,
        enabled: operation.enabled,
        toolId: operation.toolId,
        type: operation.type,
        params: operation.params,
        ...patch,
      }
      await editOperation(operationId!, input)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
      const ops = await listOperations()
      setOperation(ops.find((o) => o.id === operationId) ?? null)
    } catch (e) { handleError(e) }
  }

  if (operation.type === 'pocket') {
    const params = operation.params as PocketParams
    return (
      <div key={operation.id} style={{ padding: '0.5rem', borderTop: '1px solid #ccc' }}>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-tool">Tool</label>
          <select id="oef-tool" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-depth">Floor depth (mm)</label>
          <input id="oef-depth" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepdown">Step-down (mm)</label>
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepover">Stepover (%)</label>
          <input id="oef-stepover" type="number" defaultValue={params.stepoverPercent}
            onBlur={(e) => void save({ params: { ...params, stepoverPercent: parseFloat(e.target.value) } })} />
        </div>
      </div>
    )
  }

  if (operation.type === 'profile') {
    const params = operation.params as ProfileParams
    return (
      <div key={operation.id} style={{ padding: '0.5rem', borderTop: '1px solid #ccc' }}>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-tool">Tool</label>
          <select id="oef-tool" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-depth">Floor depth (mm)</label>
          <input id="oef-depth" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepdown">Step-down (mm)</label>
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-compensation">Compensation side</label>
          <select id="oef-compensation" value={params.compensationSide}
            onChange={(e) => void save({ params: { ...params, compensationSide: e.target.value as ProfileParams['compensationSide'] } })}>
            <option value="left">Left</option>
            <option value="center">Center</option>
            <option value="right">Right</option>
          </select>
        </div>
      </div>
    )
  }

  if (operation.type === 'drill') {
    const params = operation.params as DrillParams
    const points = params.points
    return (
      <div key={operation.id} style={{ padding: '0.5rem', borderTop: '1px solid #ccc' }}>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-tool">Tool</label>
          <select id="oef-tool" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-depth">Depth (mm)</label>
          <input id="oef-depth" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-peck-depth">Peck depth (mm)</label>
          <input id="oef-peck-depth" type="number" defaultValue={params.peckDepth ?? ''}
            onBlur={(e) => void save({ params: { ...params, peckDepth: e.target.value === '' ? undefined : parseFloat(e.target.value) } })} />
        </div>
        <div>
          {points.map((pt, i) => (
            <div key={i}>
              <input id={`oef-point-x-${i}`} type="number" defaultValue={pt.x}
                onBlur={(e) => {
                  const updated = points.map((p, j) => j === i ? { ...p, x: parseFloat(e.target.value) } : p)
                  void save({ params: { ...params, points: updated } })
                }} />
              <input id={`oef-point-y-${i}`} type="number" defaultValue={pt.y}
                onBlur={(e) => {
                  const updated = points.map((p, j) => j === i ? { ...p, y: parseFloat(e.target.value) } : p)
                  void save({ params: { ...params, points: updated } })
                }} />
              <button onClick={() => void save({ params: { ...params, points: points.filter((_, j) => j !== i) } })}>Remove</button>
            </div>
          ))}
          <button onClick={() => void save({ params: { ...params, points: [...points, { x: 0, y: 0 }] } })}>Add point</button>
        </div>
      </div>
    )
  }

  return <div style={{ padding: '0.5rem', color: '#888' }}>Parameters coming soon</div>
}
