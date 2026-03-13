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
import { addOperation, deleteOperation, editOperation, listOperations, reorderOperations } from '../../api/operations'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import { calculateToolpath, getToolpathGeometry, listenToolpathProgress } from '../../api/toolpath'
import { OperationEditorForm } from './OperationEditorForm'
import type { OperationInput, DrillParams, ZLevelRoughingParams, ZLevelFinishingParams, AdaptiveClearingParams, ToolpathProgressEvent } from '../../api/types'

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
  const [calculatingId, setCalculatingId] = useState<string | null>(null)
  const [progress, setProgress] = useState<number>(0)

  useEffect(() => {
    let active = true
    let unlisten: (() => void) | null = null
    listenToolpathProgress((event: ToolpathProgressEvent) => {
      if (event.operationId === calculatingId) {
        setProgress(event.percent)
      }
    }).then((fn) => {
      if (active) { unlisten = fn } else { fn() }
    }).catch(handleError)
    return () => { active = false; unlisten?.() }
  }, [calculatingId])

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
    setProgress(0)
    setCalculatingId(id)
    try {
      const stats = await calculateToolpath(id)
      const geometry = await getToolpathGeometry(id)
      setToolpathGeometry(geometry)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
      pushNotification(
        `Toolpath: ${stats.totalPassCount} passes, ${stats.totalPointCount} pts, ${stats.totalPathLengthMm.toFixed(1)} mm`
      )
    } catch (e) { handleError(e) }
    finally { setCalculatingId(null) }
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

  // ── Reorder ───────────────────────────────────────────────────────────────

  async function handleReorder(id: string, direction: 'up' | 'down') {
    const idx = operations.findIndex((o) => o.id === id)
    if (idx < 0) return
    const newIds = operations.map((o) => o.id)
    const swapIdx = direction === 'up' ? idx - 1 : idx + 1
    if (swapIdx < 0 || swapIdx >= newIds.length) return
    ;[newIds[idx], newIds[swapIdx]] = [newIds[swapIdx], newIds[idx]]
    try {
      await reorderOperations(newIds)
      const snapshot = await getProjectSnapshot()
      setSnapshot(snapshot)
    } catch (e) { handleError(e) }
  }

  // ── Add ───────────────────────────────────────────────────────────────────

  async function handleAdd(type: 'profile' | 'pocket' | 'drill' | 'z_level_roughing' | 'z_level_finishing' | 'adaptive_clearing') {
    const tool = tools[0]
    if (!tool) return

    let input: OperationInput
    if (type === 'profile') {
      input = { name: 'New profile', toolId: tool.id, type, params: { depth: 1.0, stepdown: 0.5, compensationSide: 'left' } }
    } else if (type === 'pocket') {
      input = { name: 'New pocket', toolId: tool.id, type, params: { depth: 1.0, stepdown: 0.5, stepoverPercent: 50.0 } }
    } else if (type === 'z_level_roughing') {
      input = {
        name: 'Z-Level Roughing',
        toolId: tool.id,
        type: 'z_level_roughing',
        params: { depth: 5.0, stepdown: 1.0, stepover: 0.5 } as ZLevelRoughingParams,
      }
    } else if (type === 'z_level_finishing') {
      input = {
        name: 'Z-Level Finishing',
        toolId: tool.id,
        type: 'z_level_finishing',
        params: { depth: 5.0, stepdown: 1.0, finishingAllowance: 0.1, springPass: false, restMachining: false } as ZLevelFinishingParams,
      }
    } else if (type === 'adaptive_clearing') {
      input = {
        name: 'Adaptive Clearing',
        toolId: tool.id,
        type: 'adaptive_clearing',
        params: { depth: 5.0, stepdown: 1.0, optimalLoad: 0.25, stepoverPercent: 50 } as AdaptiveClearingParams,
      }
    } else {
      input = { name: 'New drill', toolId: tool.id, type, params: { depth: 10.0, points: [] } }
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
            {op.needsRecalculate && (
              <span style={{ color: '#b45309', fontSize: '0.7em' }} aria-label="stale">
                (stale)
              </span>
            )}
            <span style={{ fontSize: '0.75em', color: '#666' }}>{op.operationType}</span>
            <button
              onClick={(e) => { e.stopPropagation(); void handleReorder(op.id, 'up') }}
              disabled={operations.indexOf(op) === 0}
              aria-label={`Move up ${op.name}`}
            >▲</button>
            <button
              onClick={(e) => { e.stopPropagation(); void handleReorder(op.id, 'down') }}
              disabled={operations.indexOf(op) === operations.length - 1}
              aria-label={`Move down ${op.name}`}
            >▼</button>
            <button
              onClick={(e) => { e.stopPropagation(); void handleDelete(op.id) }}
              aria-label={`Delete ${op.name}`}
            >
              ✕
            </button>
            <button
              onClick={(e) => { e.stopPropagation(); void handleCalculate(op.id) }}
              disabled={!stock || (op.operationType === 'drill' ? (drillPointCounts[op.id] ?? 0) < 1 : false) || calculatingId !== null}
              aria-label={`Calculate ${op.name}`}
            >
              {calculatingId === op.id ? '…' : 'Calc'}
            </button>
            {calculatingId === op.id && (
              <progress value={progress} max={100} style={{ width: '60px' }} aria-label={`Progress for ${op.name}`} />
            )}
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
        <button
          onClick={() => void handleAdd('z_level_roughing')}
          disabled={noTools}
        >
          + Z-Level Roughing
        </button>
        <button
          onClick={() => void handleAdd('z_level_finishing')}
          disabled={noTools}
        >
          + Z-Level Finishing
        </button>
        <button
          onClick={() => void handleAdd('adaptive_clearing')}
          disabled={noTools}
        >
          + Adaptive Clearing
        </button>
      </div>
    </div>
  )
}
