/**
 * FlowlineFinishingEditor — form fields for the Flowline Finishing operation.
 *
 * Receives params and an onSave callback that accepts a partial update.
 * The parent merges the partial into the full params before persisting.
 * Face selection is handled internally via the viewport store.
 */

import { useViewportStore } from '../../store/viewportStore'
import { getModelFaces } from '../../api/geometry'
import type { FlowlineFinishingParams, FlowlineDirection } from '../../api/types'

interface Props {
  params: FlowlineFinishingParams
  onSave: (params: Partial<FlowlineFinishingParams>) => void
}

export default function FlowlineFinishingEditor({ params, onSave }: Props) {
  const selectionMode = useViewportStore((s) => s.selectionMode)
  const selectedFps = useViewportStore((s) => s.selectedFaceFingerprints)
  const setSelectionMode = useViewportStore((s) => s.setSelectionMode)
  const setFaceDescriptors = useViewportStore((s) => s.setFaceDescriptors)
  const clearFaceSelection = useViewportStore((s) => s.clearFaceSelection)

  async function handleSelectFaces() {
    const faces = await getModelFaces()
    setFaceDescriptors(faces)
    if (params.geometry?.length) {
      useViewportStore.getState().clearFaceSelection()
      params.geometry.forEach(fp => useViewportStore.getState().toggleFaceSelection(fp))
    } else {
      clearFaceSelection()
    }
    setSelectionMode(true)
  }

  function handleDoneSelecting() {
    setSelectionMode(false)
    const fps = useViewportStore.getState().selectedFaceFingerprints
    onSave({ geometry: fps.length ? fps : null })
  }

  function handleClearGeometry() {
    clearFaceSelection()
    onSave({ geometry: null })
  }

  return (
    <>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-direction">UV direction</label>
        <select id="ffe-direction" value={params.direction}
          onChange={(e) => onSave({ direction: e.target.value as FlowlineDirection })}>
          <option value="u">U</option>
          <option value="v">V</option>
        </select>
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-stepover">Stepover (mm)</label>
        <input id="ffe-stepover" type="number" min="0.001" step="0.01" defaultValue={params.stepover}
          onBlur={(e) => onSave({ stepover: parseFloat(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-allowance">Allowance (mm)</label>
        <input id="ffe-allowance" type="number" min="0" step="0.01" defaultValue={params.allowance}
          onBlur={(e) => onSave({ allowance: parseFloat(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-tool-diameter">Tool diameter (mm)</label>
        <input id="ffe-tool-diameter" type="number" min="0.1" step="0.1" defaultValue={params.toolDiameter}
          onBlur={(e) => onSave({ toolDiameter: parseFloat(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-arc-lead-in">Arc lead-in radius (mm)</label>
        <input id="ffe-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-arc-lead-out">Arc lead-out radius (mm)</label>
        <input id="ffe-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-helical-radius">Helical entry radius (mm)</label>
        <input id="ffe-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
          onBlur={(e) => onSave({ helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-helical-pitch">Helical entry pitch (mm)</label>
        <input id="ffe-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
          onBlur={(e) => onSave({ helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) })} />
      </div>
      <div style={{ marginBottom: '0.25rem' }}>
        <label htmlFor="ffe-ramp-angle">Ramp entry angle (°)</label>
        <input id="ffe-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
          onBlur={(e) => onSave({ rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) })} />
      </div>
      <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.25rem' }}>
        <div style={{ fontSize: '0.8em', color: '#555', marginBottom: '0.25rem' }}>
          {selectionMode
            ? `${selectedFps.length} face(s) selected`
            : params.geometry?.length ? `${params.geometry.length} face(s) selected` : 'Stock boundary (default)'
          }
        </div>
        {selectionMode ? (
          <button onClick={() => handleDoneSelecting()}>Done Selecting</button>
        ) : (
          <button onClick={() => void handleSelectFaces()}>Select Faces</button>
        )}
        {!selectionMode && params.geometry?.length ? (
          <button onClick={() => handleClearGeometry()}>Clear</button>
        ) : null}
      </div>
    </>
  )
}
