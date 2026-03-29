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

import { useEffect, useRef, useState } from 'react'
import { useTools, useProjectStore, useStock, useOperations } from '../../store/projectStore'
import { editOperation, listOperations } from '../../api/operations'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { Operation, OperationInput, PocketParams, ProfileParams, DrillParams, ZLevelRoughingParams, ZLevelFinishingParams, AdaptiveClearingParams, ParallelFinishingParams, ScallopFinishingParams, FlowlineFinishingParams, PencilMillingParams } from '../../api/types'
import ParallelFinishingEditor from './ParallelFinishingEditor'
import { MaterialSelectorField } from './MaterialSelectorField'
import ScallopFinishingEditor from './ScallopFinishingEditor'
import FlowlineFinishingEditor from './FlowlineFinishingEditor'
import PencilMillingEditor from './PencilMillingEditor'
import GougeCheckPanel from './GougeCheckPanel'
import { useViewportStore } from '../../store/viewportStore'
import { getModelFaces, detectHoles } from '../../api/geometry'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { FormField } from '@/components/ui/form-field'
import { Separator } from '@/components/ui/separator'

const OPERATION_CATEGORY: Record<string, string> = {
  pocket: 'roughing',
  adaptive_clearing: 'roughing',
  z_level_roughing: 'roughing',
  profile: 'roughing',
  drill: 'drilling',
  z_level_finishing: 'finishing',
  parallelFinishing: 'finishing',
  scallopFinishing: 'finishing',
  flowlineFinishing: 'finishing',
  pencilMilling: 'finishing',
}

interface Props {
  operationId: string | null
}

export function OperationEditorForm({ operationId }: Props) {
  const tools = useTools()
  const stock = useStock()
  const allOperations = useOperations()
  const setSnapshot = useProjectStore((s) => s.setSnapshot)
  const pushNotification = useProjectStore((s) => s.pushNotification)
  const [operation, setOperation] = useState<Operation | null>(null)
  const [workpieceMaterial, setWorkpieceMaterial] = useState<string | null>(null)
  const workpieceMaterialRef = useRef<string | null>(null)
  const selectionMode = useViewportStore((s) => s.selectionMode)
  const selectedFps = useViewportStore((s) => s.selectedFaceFingerprints)
  const setSelectionMode = useViewportStore((s) => s.setSelectionMode)
  const setFaceDescriptors = useViewportStore((s) => s.setFaceDescriptors)
  const clearFaceSelection = useViewportStore((s) => s.clearFaceSelection)
  const toolpathGeometry = useViewportStore((s) => s.toolpathGeometry)
  const [toolpathVersion, setToolpathVersion] = useState(0)
  useEffect(() => { setToolpathVersion((v) => v + 1) }, [toolpathGeometry])

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  useEffect(() => {
    if (!operationId) { setOperation(null); return }
    listOperations()
      .then((ops) => {
        const op = ops.find((o) => o.id === operationId) ?? null
        setOperation(op)
        workpieceMaterialRef.current = op?.workpieceMaterial ?? null
        setWorkpieceMaterial(op?.workpieceMaterial ?? null)
      })
      .catch(handleError)
    return () => { setSelectionMode(false) }
  }, [operationId])

  if (!operationId || !operation) {
    return <div className="p-2 text-sm text-muted-foreground">Select an operation to edit</div>
  }

  const currentTool = tools.find((t) => t.id === operation.toolId) ?? null

  function materialSelectorField() {
    return (
      <MaterialSelectorField
        currentMaterialId={workpieceMaterial}
        toolMaterial={currentTool?.material ?? null}
        operationCategory={OPERATION_CATEGORY[operation!.type] ?? 'roughing'}
        onMaterialChange={(id) => {
          workpieceMaterialRef.current = id
          setWorkpieceMaterial(id)
          void save({ workpieceMaterial: id || undefined })
        }}
        onFeedsFetched={(entry) => {
          void save({
            workpieceMaterial: workpieceMaterialRef.current || undefined,
            spindleSpeedOverride: entry.spindleSpeedRpm,
            feedRateOverride: entry.feedRateMmpm,
          })
        }}
        onFeedsNotFound={() => pushNotification('No feed data found for this material/tool/category combination')}
      />
    )
  }

  async function handleSelectFaces() {
    const faces = await getModelFaces()
    setFaceDescriptors(faces)
    const savedGeo = (operation!.params as PocketParams | ProfileParams | ZLevelRoughingParams | ZLevelFinishingParams | AdaptiveClearingParams | ParallelFinishingParams | ScallopFinishingParams).geometry
    if (savedGeo?.length) {
      useViewportStore.getState().clearFaceSelection()
      savedGeo.forEach(fp => useViewportStore.getState().toggleFaceSelection(fp))
    } else {
      clearFaceSelection()
    }
    setSelectionMode(true)
  }

  async function handleDoneSelecting() {
    setSelectionMode(false)
    const fps = useViewportStore.getState().selectedFaceFingerprints
    await save({ params: { ...operation!.params, geometry: fps.length ? fps : null } })
  }

  async function handleClearGeometry() {
    clearFaceSelection()
    await save({ params: { ...operation!.params, geometry: null } })
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
        spindleSpeedOverride: operation.spindleSpeedOverride,
        feedRateOverride: operation.feedRateOverride,
        workpieceMaterial: operation.workpieceMaterial,
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
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Floor depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </FormField>
        <FormField label="Step-down (mm)" htmlFor="oef-stepdown">
          <Input id="oef-stepdown" className="h-7 text-xs" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: parseFloat(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-in radius (mm)" htmlFor="oef-arc-lead-in">
          <Input id="oef-arc-lead-in" className="h-7 text-xs" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-out radius (mm)" htmlFor="oef-arc-lead-out">
          <Input id="oef-arc-lead-out" className="h-7 text-xs" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry radius (mm)" htmlFor="oef-helical-radius">
          <Input id="oef-helical-radius" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry pitch (mm)" htmlFor="oef-helical-pitch">
          <Input id="oef-helical-pitch" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Ramp entry angle (\u00b0)" htmlFor="oef-ramp-angle">
          <Input id="oef-ramp-angle" className="h-7 text-xs" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Stepover (%)" htmlFor="oef-stepover">
          <Input id="oef-stepover" className="h-7 text-xs" type="number" defaultValue={params.stepoverPercent}
            onBlur={(e) => void save({ params: { ...params, stepoverPercent: parseFloat(e.target.value) } })} />
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <p className="text-xs text-muted-foreground">
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as PocketParams | ProfileParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </p>
          <div className="flex gap-1.5">
            {selectionMode ? (
              <Button variant="outline" size="sm" onClick={() => void handleDoneSelecting()}>Done Selecting</Button>
            ) : (
              <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>Select Faces</Button>
            )}
            {!selectionMode && (operation.params as PocketParams | ProfileParams).geometry?.length ? (
              <Button variant="ghost" size="sm" onClick={() => void handleClearGeometry()}>Clear</Button>
            ) : null}
          </div>
        </div>
      </div>
    )
  }

  if (operation.type === 'profile') {
    const params = operation.params as ProfileParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Floor depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </FormField>
        <FormField label="Step-down (mm)" htmlFor="oef-stepdown">
          <Input id="oef-stepdown" className="h-7 text-xs" type="number" defaultValue={params.stepdown ?? ''}
            onBlur={(e) => void save({ params: { ...params, stepdown: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-in radius (mm)" htmlFor="oef-arc-lead-in">
          <Input id="oef-arc-lead-in" className="h-7 text-xs" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-out radius (mm)" htmlFor="oef-arc-lead-out">
          <Input id="oef-arc-lead-out" className="h-7 text-xs" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry radius (mm)" htmlFor="oef-helical-radius">
          <Input id="oef-helical-radius" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry pitch (mm)" htmlFor="oef-helical-pitch">
          <Input id="oef-helical-pitch" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Ramp entry angle (\u00b0)" htmlFor="oef-ramp-angle">
          <Input id="oef-ramp-angle" className="h-7 text-xs" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Compensation side" htmlFor="oef-compensation">
          <select id="oef-compensation" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={params.compensationSide}
            onChange={(e) => void save({ params: { ...params, compensationSide: e.target.value as ProfileParams['compensationSide'] } })}>
            <option value="left">Left</option>
            <option value="center">Center</option>
            <option value="right">Right</option>
          </select>
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <p className="text-xs text-muted-foreground">
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as PocketParams | ProfileParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </p>
          <div className="flex gap-1.5">
            {selectionMode ? (
              <Button variant="outline" size="sm" onClick={() => void handleDoneSelecting()}>Done Selecting</Button>
            ) : (
              <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>Select Faces</Button>
            )}
            {!selectionMode && (operation.params as PocketParams | ProfileParams).geometry?.length ? (
              <Button variant="ghost" size="sm" onClick={() => void handleClearGeometry()}>Clear</Button>
            ) : null}
          </div>
        </div>
      </div>
    )
  }

  if (operation.type === 'drill') {
    const params = operation.params as DrillParams
    const points = params.points
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: parseFloat(e.target.value) } })} />
        </FormField>
        <FormField label="Peck depth (mm)" htmlFor="oef-peck-depth">
          <Input id="oef-peck-depth" className="h-7 text-xs" type="number" defaultValue={params.peckDepth ?? ''}
            onBlur={(e) => void save({ params: { ...params, peckDepth: e.target.value === '' ? undefined : parseFloat(e.target.value) } })} />
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mb-1.5">
          <Button variant="outline" size="sm" onClick={() => {
            void (async () => {
              try {
                const holes = await detectHoles()
                if (holes.length === 0) {
                  pushNotification('No holes detected')
                  return
                }
                const mappedPoints = holes.map((h) => ({ x: h.centerX, y: h.centerY }))
                if (points.length > 0) {
                  if (!window.confirm(`Replace existing drill points with ${holes.length} detected holes?`)) return
                }
                void save({ params: { ...params, points: mappedPoints } })
              } catch (e) {
                handleError(e)
              }
            })()
          }}>Detect Holes</Button>
        </div>
        <div>
          {points.map((pt, i) => (
            <div key={i} className="flex items-center gap-1.5 mb-1">
              <Input id={`oef-point-x-${i}`} className="h-7 text-xs" type="number" defaultValue={pt.x}
                onBlur={(e) => {
                  const updated = points.map((p, j) => j === i ? { ...p, x: parseFloat(e.target.value) } : p)
                  void save({ params: { ...params, points: updated } })
                }} />
              <Input id={`oef-point-y-${i}`} className="h-7 text-xs" type="number" defaultValue={pt.y}
                onBlur={(e) => {
                  const updated = points.map((p, j) => j === i ? { ...p, y: parseFloat(e.target.value) } : p)
                  void save({ params: { ...params, points: updated } })
                }} />
              <Button variant="outline" size="sm" onClick={() => void save({ params: { ...params, points: points.filter((_, j) => j !== i) } })}>Remove</Button>
            </div>
          ))}
          <Button variant="outline" size="sm" onClick={() => void save({ params: { ...params, points: [...points, { x: 0, y: 0 }] } })}>Add point</Button>
        </div>
      </div>
    )
  }

  if (operation.type === 'z_level_roughing') {
    const params = operation.params as ZLevelRoughingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Stepdown (mm)" htmlFor="oef-stepdown">
          <Input id="oef-stepdown" className="h-7 text-xs" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Stepover (%)" htmlFor="oef-stepover">
          {/* stepover stored as fraction 0-1; displayed and edited as percentage */}
          <Input id="oef-stepover" className="h-7 text-xs" type="number" defaultValue={params.stepover * 100}
            onBlur={(e) => void save({ params: { ...params, stepover: Number(e.target.value) / 100 } })} />
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <p className="text-xs text-muted-foreground">
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as ZLevelRoughingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </p>
          <div className="flex gap-1.5">
            {selectionMode ? (
              <Button variant="outline" size="sm" onClick={() => void handleDoneSelecting()}>Done Selecting</Button>
            ) : (
              <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>Select Faces</Button>
            )}
            {!selectionMode && (operation.params as ZLevelRoughingParams).geometry?.length ? (
              <Button variant="ghost" size="sm" onClick={() => void handleClearGeometry()}>Clear</Button>
            ) : null}
          </div>
        </div>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
      </div>
    )
  }

  if (operation.type === 'z_level_finishing') {
    const params = operation.params as ZLevelFinishingParams
    const roughingOps = allOperations.filter((o) => o.operationType === 'z_level_roughing')
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Stepdown (mm)" htmlFor="oef-stepdown">
          <Input id="oef-stepdown" className="h-7 text-xs" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Finishing allowance (mm)" htmlFor="oef-finishing-allowance">
          <Input id="oef-finishing-allowance" className="h-7 text-xs" type="number" defaultValue={params.finishingAllowance}
            onBlur={(e) => void save({ params: { ...params, finishingAllowance: Number(e.target.value) } })} />
        </FormField>
        <div className="mb-1.5 flex items-center gap-2">
          <Checkbox
            id="oef-spring-pass"
            checked={params.springPass}
            onCheckedChange={(checked) => void save({ params: { ...params, springPass: checked === true } })}
            className="h-3.5 w-3.5"
          />
          <label htmlFor="oef-spring-pass" className="text-xs text-muted-foreground">Spring pass</label>
        </div>
        <FormField label="Arc lead-in radius (mm)" htmlFor="oef-arc-lead-in">
          <Input id="oef-arc-lead-in" className="h-7 text-xs" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-out radius (mm)" htmlFor="oef-arc-lead-out">
          <Input id="oef-arc-lead-out" className="h-7 text-xs" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry radius (mm)" htmlFor="oef-helical-radius">
          <Input id="oef-helical-radius" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry pitch (mm)" htmlFor="oef-helical-pitch">
          <Input id="oef-helical-pitch" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Ramp entry angle (\u00b0)" htmlFor="oef-ramp-angle">
          <Input id="oef-ramp-angle" className="h-7 text-xs" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <p className="text-xs text-muted-foreground">
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as ZLevelFinishingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </p>
          <div className="flex gap-1.5">
            {selectionMode ? (
              <Button variant="outline" size="sm" onClick={() => void handleDoneSelecting()}>Done Selecting</Button>
            ) : (
              <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>Select Faces</Button>
            )}
            {!selectionMode && (operation.params as ZLevelFinishingParams).geometry?.length ? (
              <Button variant="ghost" size="sm" onClick={() => void handleClearGeometry()}>Clear</Button>
            ) : null}
          </div>
        </div>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <div className="flex items-center gap-2">
            <Checkbox
              id="oef-rest-machining"
              checked={params.restMachining}
              onCheckedChange={(checked) => void save({ params: { ...params, restMachining: checked === true, ...(!checked ? { restMachiningReferenceId: undefined } : {}) } })}
              className="h-3.5 w-3.5"
            />
            <label htmlFor="oef-rest-machining" className="text-xs text-muted-foreground">Rest machining</label>
          </div>
          {params.restMachining && (
            <FormField label="Reference operation" htmlFor="oef-rest-ref">
              <select id="oef-rest-ref" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={params.restMachiningReferenceId ?? ''}
                onChange={(e) => void save({ params: { ...params, restMachiningReferenceId: e.target.value || undefined } })}>
                <option value="">— Select —</option>
                {roughingOps.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
              </select>
            </FormField>
          )}
        </div>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
      </div>
    )
  }

  if (operation.type === 'adaptive_clearing') {
    const params = operation.params as AdaptiveClearingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FormField label="Depth (mm)" htmlFor="oef-depth">
          <Input id="oef-depth" className="h-7 text-xs" type="number" defaultValue={params.depth}
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Stepdown (mm)" htmlFor="oef-stepdown">
          <Input id="oef-stepdown" className="h-7 text-xs" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </FormField>
        <FormField label="Optimal load (%)" htmlFor="oef-optimal-load">
          {/* optimalLoad stored as fraction 0-1; displayed and edited as percentage */}
          <Input id="oef-optimal-load" className="h-7 text-xs" type="number" defaultValue={params.optimalLoad * 100}
            onBlur={(e) => void save({ params: { ...params, optimalLoad: Number(e.target.value) / 100 } })} />
        </FormField>
        <FormField label="Stepover (%)" htmlFor="oef-stepover">
          <Input id="oef-stepover" className="h-7 text-xs" type="number" defaultValue={params.stepoverPercent}
            onBlur={(e) => void save({ params: { ...params, stepoverPercent: parseFloat(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-in radius (mm)" htmlFor="oef-arc-lead-in">
          <Input id="oef-arc-lead-in" className="h-7 text-xs" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Arc lead-out radius (mm)" htmlFor="oef-arc-lead-out">
          <Input id="oef-arc-lead-out" className="h-7 text-xs" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry radius (mm)" htmlFor="oef-helical-radius">
          <Input id="oef-helical-radius" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Helical entry pitch (mm)" htmlFor="oef-helical-pitch">
          <Input id="oef-helical-pitch" className="h-7 text-xs" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        <FormField label="Ramp entry angle (\u00b0)" htmlFor="oef-ramp-angle">
          <Input id="oef-ramp-angle" className="h-7 text-xs" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </FormField>
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2 space-y-1.5">
          <Separator />
          <p className="text-xs text-muted-foreground">
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as AdaptiveClearingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </p>
          <div className="flex gap-1.5">
            {selectionMode ? (
              <Button variant="outline" size="sm" onClick={() => void handleDoneSelecting()}>Done Selecting</Button>
            ) : (
              <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>Select Faces</Button>
            )}
            {!selectionMode && (operation.params as AdaptiveClearingParams).geometry?.length ? (
              <Button variant="ghost" size="sm" onClick={() => void handleClearGeometry()}>Clear</Button>
            ) : null}
          </div>
        </div>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
      </div>
    )
  }

  if (operation.type === 'parallelFinishing') {
    const params = operation.params as ParallelFinishingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <ParallelFinishingEditor
          params={params}
          onSave={(partial) => void save({ params: { ...params, ...partial } })}
        />
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
        {toolpathGeometry && (
          <GougeCheckPanel operationId={operation.id} toolpathVersion={toolpathVersion} />
        )}
      </div>
    )
  }

  if (operation.type === 'scallopFinishing') {
    const params = operation.params as ScallopFinishingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <ScallopFinishingEditor
          params={params}
          onSave={(partial) => void save({ params: { ...params, ...partial } })}
        />
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
        {toolpathGeometry && (
          <GougeCheckPanel operationId={operation.id} toolpathVersion={toolpathVersion} />
        )}
      </div>
    )
  }

  if (operation.type === 'flowlineFinishing') {
    const params = operation.params as FlowlineFinishingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <FlowlineFinishingEditor
          params={params}
          onSave={(partial) => void save({ params: { ...params, ...partial } })}
        />
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
        {toolpathGeometry && (
          <GougeCheckPanel operationId={operation.id} toolpathVersion={toolpathVersion} />
        )}
      </div>
    )
  }

  if (operation.type === 'pencilMilling') {
    const params = operation.params as PencilMillingParams
    return (
      <div key={operation.id} className="border-t border-border p-2">
        <FormField label="Tool" htmlFor="oef-tool">
          <select id="oef-tool" className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground" value={operation.toolId} onChange={(e) => void save({ toolId: e.target.value })}>
            {tools.map((t) => <option key={t.id} value={t.id}>{t.name}</option>)}
          </select>
        </FormField>
        <PencilMillingEditor
          params={params}
          onSave={(partial) => void save({ params: { ...params, ...partial } })}
        />
        {materialSelectorField()}
        <FormField label="Spindle speed override (RPM)" htmlFor="oef-spindle-override">
          <Input id="oef-spindle-override" className="h-7 text-xs" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </FormField>
        <FormField label="Feed rate override (mm/min)" htmlFor="oef-feed-override">
          <Input id="oef-feed-override" className="h-7 text-xs" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </FormField>
        <div className="mt-2">
          <Button variant="outline" size="sm" disabled={stock === null}>Calculate</Button>
        </div>
        {toolpathGeometry && (
          <GougeCheckPanel operationId={operation.id} toolpathVersion={toolpathVersion} />
        )}
      </div>
    )
  }

  return <div className="p-2 text-sm text-muted-foreground">Parameters coming soon</div>
}
