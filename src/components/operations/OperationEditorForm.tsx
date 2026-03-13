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
import { useTools, useProjectStore, useStock, useOperations } from '../../store/projectStore'
import { editOperation, listOperations } from '../../api/operations'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { Operation, OperationInput, PocketParams, ProfileParams, DrillParams, ZLevelRoughingParams, ZLevelFinishingParams, AdaptiveClearingParams } from '../../api/types'
import { useViewportStore } from '../../store/viewportStore'
import { getModelFaces, detectHoles } from '../../api/geometry'

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
  const selectionMode = useViewportStore((s) => s.selectionMode)
  const selectedFps = useViewportStore((s) => s.selectedFaceFingerprints)
  const setSelectionMode = useViewportStore((s) => s.setSelectionMode)
  const setFaceDescriptors = useViewportStore((s) => s.setFaceDescriptors)
  const clearFaceSelection = useViewportStore((s) => s.clearFaceSelection)

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  useEffect(() => {
    if (!operationId) { setOperation(null); return }
    listOperations()
      .then((ops) => setOperation(ops.find((o) => o.id === operationId) ?? null))
      .catch(handleError)
    return () => { setSelectionMode(false) }
  }, [operationId])

  if (!operationId || !operation) {
    return <div style={{ padding: '0.5rem', color: '#888' }}>Select an operation to edit</div>
  }

  async function handleSelectFaces() {
    const faces = await getModelFaces()
    setFaceDescriptors(faces)
    const savedGeo = (operation!.params as PocketParams | ProfileParams | ZLevelRoughingParams | ZLevelFinishingParams | AdaptiveClearingParams).geometry
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
          <label htmlFor="oef-arc-lead-in">Arc lead-in radius (mm)</label>
          <input id="oef-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-out">Arc lead-out radius (mm)</label>
          <input id="oef-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-radius">Helical entry radius (mm)</label>
          <input id="oef-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-pitch">Helical entry pitch (mm)</label>
          <input id="oef-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-ramp-angle">Ramp entry angle (°)</label>
          <input id="oef-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepover">Stepover (%)</label>
          <input id="oef-stepover" type="number" defaultValue={params.stepoverPercent}
            onBlur={(e) => void save({ params: { ...params, stepoverPercent: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as PocketParams | ProfileParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </div>
          {selectionMode ? (
            <button onClick={() => void handleDoneSelecting()}>Done Selecting</button>
          ) : (
            <button onClick={() => void handleSelectFaces()}>Select Faces</button>
          )}
          {!selectionMode && (operation.params as PocketParams | ProfileParams).geometry?.length ? (
            <button onClick={() => void handleClearGeometry()}>Clear</button>
          ) : null}
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
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown ?? ''}
            onBlur={(e) => void save({ params: { ...params, stepdown: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-in">Arc lead-in radius (mm)</label>
          <input id="oef-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-out">Arc lead-out radius (mm)</label>
          <input id="oef-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-radius">Helical entry radius (mm)</label>
          <input id="oef-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-pitch">Helical entry pitch (mm)</label>
          <input id="oef-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-ramp-angle">Ramp entry angle (°)</label>
          <input id="oef-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
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
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as PocketParams | ProfileParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </div>
          {selectionMode ? (
            <button onClick={() => void handleDoneSelecting()}>Done Selecting</button>
          ) : (
            <button onClick={() => void handleSelectFaces()}>Select Faces</button>
          )}
          {!selectionMode && (operation.params as PocketParams | ProfileParams).geometry?.length ? (
            <button onClick={() => void handleClearGeometry()}>Clear</button>
          ) : null}
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
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <button onClick={() => {
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
          }}>Detect Holes</button>
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

  if (operation.type === 'z_level_roughing') {
    const params = operation.params as ZLevelRoughingParams
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
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepdown">Stepdown (mm)</label>
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          {/* stepover stored as fraction 0–1; displayed and edited as percentage */}
          <label htmlFor="oef-stepover">Stepover (%)</label>
          <input id="oef-stepover" type="number" defaultValue={params.stepover * 100}
            onBlur={(e) => void save({ params: { ...params, stepover: Number(e.target.value) / 100 } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as ZLevelRoughingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </div>
          {selectionMode ? (
            <button onClick={() => void handleDoneSelecting()}>Done Selecting</button>
          ) : (
            <button onClick={() => void handleSelectFaces()}>Select Faces</button>
          )}
          {!selectionMode && (operation.params as ZLevelRoughingParams).geometry?.length ? (
            <button onClick={() => void handleClearGeometry()}>Clear</button>
          ) : null}
        </div>
        <div style={{ marginTop: '0.5rem' }}>
          <button disabled={stock === null}>Calculate</button>
        </div>
      </div>
    )
  }

  if (operation.type === 'z_level_finishing') {
    const params = operation.params as ZLevelFinishingParams
    const roughingOps = allOperations.filter((o) => o.operationType === 'z_level_roughing')
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
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepdown">Stepdown (mm)</label>
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-finishing-allowance">Finishing allowance (mm)</label>
          <input id="oef-finishing-allowance" type="number" defaultValue={params.finishingAllowance}
            onBlur={(e) => void save({ params: { ...params, finishingAllowance: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label>
            <input type="checkbox" checked={params.springPass}
              onChange={(e) => void save({ params: { ...params, springPass: e.target.checked } })} />
            {' '}Spring pass
          </label>
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-in">Arc lead-in radius (mm)</label>
          <input id="oef-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-out">Arc lead-out radius (mm)</label>
          <input id="oef-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-radius">Helical entry radius (mm)</label>
          <input id="oef-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-pitch">Helical entry pitch (mm)</label>
          <input id="oef-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-ramp-angle">Ramp entry angle (°)</label>
          <input id="oef-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as ZLevelFinishingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </div>
          {selectionMode ? (
            <button onClick={() => void handleDoneSelecting()}>Done Selecting</button>
          ) : (
            <button onClick={() => void handleSelectFaces()}>Select Faces</button>
          )}
          {!selectionMode && (operation.params as ZLevelFinishingParams).geometry?.length ? (
            <button onClick={() => void handleClearGeometry()}>Clear</button>
          ) : null}
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <label>
            <input type="checkbox" checked={params.restMachining}
              onChange={(e) => void save({ params: { ...params, restMachining: e.target.checked, ...(!e.target.checked ? { restMachiningReferenceId: undefined } : {}) } })} />
            {' '}Rest machining
          </label>
          {params.restMachining && (
            <div style={{ marginTop: '0.25rem' }}>
              <label htmlFor="oef-rest-ref">Reference operation</label>
              <select id="oef-rest-ref" value={params.restMachiningReferenceId ?? ''}
                onChange={(e) => void save({ params: { ...params, restMachiningReferenceId: e.target.value || undefined } })}>
                <option value="">— Select —</option>
                {roughingOps.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
              </select>
            </div>
          )}
        </div>
        <div style={{ marginTop: '0.5rem' }}>
          <button disabled={stock === null}>Calculate</button>
        </div>
      </div>
    )
  }

  if (operation.type === 'adaptive_clearing') {
    const params = operation.params as AdaptiveClearingParams
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
            onBlur={(e) => void save({ params: { ...params, depth: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepdown">Stepdown (mm)</label>
          <input id="oef-stepdown" type="number" defaultValue={params.stepdown}
            onBlur={(e) => void save({ params: { ...params, stepdown: Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          {/* optimalLoad stored as fraction 0–1; displayed and edited as percentage */}
          <label htmlFor="oef-optimal-load">Optimal load (%)</label>
          <input id="oef-optimal-load" type="number" defaultValue={params.optimalLoad * 100}
            onBlur={(e) => void save({ params: { ...params, optimalLoad: Number(e.target.value) / 100 } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-stepover">Stepover (%)</label>
          <input id="oef-stepover" type="number" defaultValue={params.stepoverPercent}
            onBlur={(e) => void save({ params: { ...params, stepoverPercent: parseFloat(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-in">Arc lead-in radius (mm)</label>
          <input id="oef-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-arc-lead-out">Arc lead-out radius (mm)</label>
          <input id="oef-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-radius">Helical entry radius (mm)</label>
          <input id="oef-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-helical-pitch">Helical entry pitch (mm)</label>
          <input id="oef-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
            onBlur={(e) => void save({ params: { ...params, helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-ramp-angle">Ramp entry angle (°)</label>
          <input id="oef-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
            onBlur={(e) => void save({ params: { ...params, rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) } })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-spindle-override">Spindle speed override (RPM)</label>
          <input id="oef-spindle-override" type="number" defaultValue={operation.spindleSpeedOverride ?? ''}
            onBlur={(e) => void save({ spindleSpeedOverride: e.target.value === '' ? null : parseInt(e.target.value, 10) })} />
        </div>
        <div style={{ marginBottom: '0.25rem' }}>
          <label htmlFor="oef-feed-override">Feed rate override (mm/min)</label>
          <input id="oef-feed-override" type="number" defaultValue={operation.feedRateOverride ?? ''}
            onBlur={(e) => void save({ feedRateOverride: e.target.value === '' ? null : parseFloat(e.target.value) })} />
        </div>
        <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
          <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
            {selectionMode
              ? `${selectedFps.length} face(s) selected`
              : (() => {
                  const g = (operation.params as AdaptiveClearingParams).geometry
                  return g?.length ? `${g.length} face(s) selected` : 'Stock boundary (default)'
                })()
            }
          </div>
          {selectionMode ? (
            <button onClick={() => void handleDoneSelecting()}>Done Selecting</button>
          ) : (
            <button onClick={() => void handleSelectFaces()}>Select Faces</button>
          )}
          {!selectionMode && (operation.params as AdaptiveClearingParams).geometry?.length ? (
            <button onClick={() => void handleClearGeometry()}>Clear</button>
          ) : null}
        </div>
        <div style={{ marginTop: '0.5rem' }}>
          <button disabled={stock === null}>Calculate</button>
        </div>
      </div>
    )
  }

  return <div style={{ padding: '0.5rem', color: '#888' }}>Parameters coming soon</div>
}
