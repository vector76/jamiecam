/**
 * OperationListPanel — sidebar panel listing machining operations.
 *
 * Displays the current operation list from the project store.  Each row
 * shows the operation name, type, an enable/disable toggle, and a delete
 * button.  Add buttons at the bottom create new operations of each type
 * using the first available tool; they are disabled when no tools exist.
 */

import { useState, useEffect } from 'react'
import { useOperations, useProjectStore, useSelectedOperationId, useStock, useTools } from '../../store/projectStore'
import { useViewportStore } from '../../store/viewportStore'
import { addOperation, deleteOperation, editOperation, listOperations } from '../../api/operations'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import { calculateToolpath, getToolpathGeometry } from '../../api/toolpath'
import { OperationEditorForm } from './OperationEditorForm'
import type { OperationInput, DrillParams } from '../../api/types'

export function OperationListPanel() {
  const operations = useOperations()
  const tools = useTools()
  const stock = useStock()
  const selectedOpId = useSelectedOperationId()
  const setSelectedOpId = useProjectStore((s) => s.setSelectedOperationId)
  const setSnapshot = useProjectStore((s) => s.setSnapshot)
  const pushNotification = useProjectStore((s) => s.pushNotification)
  const setToolpathGeometry = useViewportStore((s) => s.setToolpathGeometry)
  const noTools = tools.length === 0

  const [drillPointCounts, setDrillPointCounts] = useState<Record<string, number>>({})

  useEffect(() => {
    if (!operations.some(o => o.operationType === 'drill')) { setDrillPointCounts({}); return }
    listOperations().then(full => {
      const counts: Record<string, number> = {}
      for (const op of full) {
        if (op.type === 'drill') counts[op.id] = (op.params as DrillParams).points.length
      }
      setDrillPointCounts(counts)
    }).catch(handleError)
  }, [operations])

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  // ── Calculate toolpath ────────────────────────────────────────────────────

  async function handleCalculate(id: string) {
    try {
      const stats = await calculateToolpath(id)
      const geometry = await getToolpathGeometry(id)
      setToolpathGeometry(geometry)
      pushNotification(
        `Toolpath: ${stats.totalPassCount} passes, ${stats.totalPointCount} pts, ${stats.totalPathLengthMm.toFixed(1)} mm`
      )
    } catch (e) { handleError(e) }
  }

  // ── Toggle enabled ────────────────────────────────────────────────────────

  async function handleToggleEnabled(id: string, currentEnabled: boolean) {
    try {
      const ops = await listOperations()
      const full = ops.find((o) => o.id === id)
      if (!full) return
      const input: OperationInput = {
        name: full.name,
        enabled: !currentEnabled,
        toolId: full.toolId,
        type: full.type,
        params: full.params,
      }
      await editOperation(id, input)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
    } catch (e) { handleError(e) }
  }

  // ── Delete ────────────────────────────────────────────────────────────────

  async function handleDelete(id: string) {
    try {
      await deleteOperation(id)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
    } catch (e) { handleError(e) }
  }

  // ── Add ───────────────────────────────────────────────────────────────────

  async function handleAdd(type: 'profile' | 'pocket' | 'drill') {
    const tool = tools[0]
    if (!tool) return

    let params: OperationInput['params']
    if (type === 'profile') {
      params = { depth: 1.0, stepdown: 0.5, compensationSide: 'left' }
    } else if (type === 'pocket') {
      params = { depth: 1.0, stepdown: 0.5, stepoverPercent: 50.0 }
    } else {
      params = { depth: 10.0, points: [] }
    }

    const input: OperationInput = {
      name: `New ${type}`,
      toolId: tool.id,
      type,
      params,
    }

    try {
      await addOperation(input)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
    } catch (e) { handleError(e) }
  }

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div style={{ width: '240px', borderLeft: '1px solid #ccc', overflowY: 'auto', padding: '0.5rem' }}>
      <div>
        {operations.map((op) => (
          <div
            key={op.id}
            onClick={() => setSelectedOpId(op.id)}
            style={{
              display: 'flex', alignItems: 'center', gap: '0.25rem', marginBottom: '0.25rem',
              cursor: 'pointer',
              background: op.id === selectedOpId ? '#e0e8ff' : undefined,
            }}
          >
            <input
              type="checkbox"
              checked={op.enabled}
              onChange={() => void handleToggleEnabled(op.id, op.enabled)}
              onClick={(e) => e.stopPropagation()}
              aria-label={`Toggle ${op.name}`}
            />
            <span style={{ flex: 1 }}>{op.name}</span>
            <span style={{ fontSize: '0.75em', color: '#666' }}>{op.operationType}</span>
            <button
              onClick={(e) => { e.stopPropagation(); void handleDelete(op.id) }}
              aria-label={`Delete ${op.name}`}
            >
              ✕
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); void handleCalculate(op.id) }}
              disabled={!stock || (op.operationType === 'drill' ? (drillPointCounts[op.id] ?? 0) < 1 : false)}
              aria-label={`Calculate ${op.name}`}
            >
              Calc
            </button>
          </div>
        ))}
      </div>
      <OperationEditorForm operationId={selectedOpId} />
      <div style={{ display: 'flex', gap: '0.25rem', marginTop: '0.5rem' }}>
        <button
          onClick={() => void handleAdd('profile')}
          disabled={noTools}
        >
          + Profile
        </button>
        <button
          onClick={() => void handleAdd('pocket')}
          disabled={noTools}
        >
          + Pocket
        </button>
        <button
          onClick={() => void handleAdd('drill')}
          disabled={noTools}
        >
          + Drill
        </button>
      </div>
    </div>
  )
}
