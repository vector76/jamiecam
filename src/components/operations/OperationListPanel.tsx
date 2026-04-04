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
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Progress } from '@/components/ui/progress'
import { ChevronUp, ChevronDown, Trash2, Calculator, Plus } from 'lucide-react'
import type { OperationInput, DrillParams, ZLevelRoughingParams, ZLevelFinishingParams, AdaptiveClearingParams, ParallelFinishingParams, ScallopFinishingParams, FlowlineFinishingParams, PencilMillingParams, ToolpathProgressEvent } from '../../api/types'

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
    // eslint-disable-next-line react-hooks/exhaustive-deps
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
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

  async function handleAdd(type: 'profile' | 'pocket' | 'drill' | 'z_level_roughing' | 'z_level_finishing' | 'adaptive_clearing' | 'parallelFinishing' | 'scallopFinishing' | 'flowlineFinishing' | 'pencilMilling') {
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
    } else if (type === 'parallelFinishing') {
      input = {
        name: 'Parallel Finishing',
        toolId: tool.id,
        type: 'parallelFinishing',
        params: { stepover: 0.5, directionAngleDeg: 0, allowance: 0 } as ParallelFinishingParams,
      }
    } else if (type === 'scallopFinishing') {
      input = {
        name: 'Scallop Finishing',
        toolId: tool.id,
        type: 'scallopFinishing',
        params: { targetScallopHeight: 0.01, minStepover: 0.1, maxStepover: 1.0, directionAngleDeg: 0, allowance: 0, toolRadius: 3.0 } as ScallopFinishingParams,
      }
    } else if (type === 'flowlineFinishing') {
      input = {
        name: 'Flowline Finishing',
        toolId: tool.id,
        type: 'flowlineFinishing',
        params: { stepover: 0.1, direction: 'u', allowance: 0, toolDiameter: 6.0 } as FlowlineFinishingParams,
      }
    } else if (type === 'pencilMilling') {
      input = {
        name: 'Pencil Milling',
        toolId: tool.id,
        type: 'pencilMilling',
        params: { allowance: 0, toolDiameter: 6.0, curvatureThreshold: null, minPassLength: 1.0 } as PencilMillingParams,
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
    <div className="space-y-2">
      <div className="space-y-0.5">
        {operations.map((op) => (
          <div
            key={op.id}
            onClick={() => setSelectedOpId(op.id)}
            className={cn(
              'flex cursor-pointer items-center gap-1 rounded-sm px-1.5 py-1',
              op.id === selectedOpId ? 'bg-primary/10' : 'hover:bg-accent',
            )}
          >
            <Checkbox
              checked={op.enabled}
              onCheckedChange={() => void handleToggleEnabled(op.id, op.enabled)}
              onClick={(e) => e.stopPropagation()}
              aria-label={`Toggle ${op.name}`}
              className="h-3.5 w-3.5"
            />
            <span className="flex-1 truncate text-sm" title={op.name}>{op.name}</span>
            {op.needsRecalculate && (
              <span className="text-[0.65rem] text-warning" aria-label="stale">
                stale
              </span>
            )}
            <span className="text-xs text-muted-foreground">{op.operationType}</span>
            <Button
              variant="ghost"
              size="icon"
              className="h-5 w-5"
              onClick={(e) => { e.stopPropagation(); void handleReorder(op.id, 'up') }}
              disabled={operations.indexOf(op) === 0}
              aria-label={`Move up ${op.name}`}
            >
              <ChevronUp className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-5 w-5"
              onClick={(e) => { e.stopPropagation(); void handleReorder(op.id, 'down') }}
              disabled={operations.indexOf(op) === operations.length - 1}
              aria-label={`Move down ${op.name}`}
            >
              <ChevronDown className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-5 w-5 text-muted-foreground hover:text-destructive"
              onClick={(e) => { e.stopPropagation(); void handleDelete(op.id) }}
              aria-label={`Delete ${op.name}`}
            >
              <Trash2 className="h-3 w-3" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className="h-5 w-5"
              onClick={(e) => { e.stopPropagation(); void handleCalculate(op.id) }}
              disabled={!stock || (op.operationType === 'drill' ? (drillPointCounts[op.id] ?? 0) < 1 : false) || calculatingId !== null}
              aria-label={`Calculate ${op.name}`}
            >
              <Calculator className="h-3 w-3" />
            </Button>
            {calculatingId === op.id && (
              <Progress value={progress} className="h-1.5 w-14" aria-label={`Progress for ${op.name}`} />
            )}
          </div>
        ))}
      </div>
      <OperationEditorForm operationId={selectedOpId} />
      <div className="grid grid-cols-2 gap-1.5">
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('profile')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Profile
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('pocket')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Pocket
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('drill')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Drill
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('z_level_roughing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Z-Rough
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('z_level_finishing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Z-Finish
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('adaptive_clearing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Adaptive
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('parallelFinishing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Parallel
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('scallopFinishing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Scallop
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('flowlineFinishing')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Flowline
        </Button>
        <Button variant="outline" size="sm" className="text-xs" onClick={() => void handleAdd('pencilMilling')} disabled={noTools}>
          <Plus className="mr-0.5 h-3 w-3" /> Pencil
        </Button>
      </div>
    </div>
  )
}
