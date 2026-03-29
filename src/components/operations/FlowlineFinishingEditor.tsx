/**
 * FlowlineFinishingEditor — form fields for the Flowline Finishing operation.
 *
 * Receives params and an onSave callback that accepts a partial update.
 * The parent merges the partial into the full params before persisting.
 * Face selection is handled internally via the viewport store.
 */

import { Input } from '@/components/ui/input'
import { FormField } from '@/components/ui/form-field'
import { FaceSelectionBlock } from './FaceSelectionBlock'
import type { FlowlineFinishingParams, FlowlineDirection } from '../../api/types'

interface Props {
  params: FlowlineFinishingParams
  onSave: (params: Partial<FlowlineFinishingParams>) => void
}

export default function FlowlineFinishingEditor({ params, onSave }: Props) {
  return (
    <>
      <FormField label="UV direction" htmlFor="ffe-direction">
        <select
          id="ffe-direction"
          value={params.direction}
          onChange={(e) => onSave({ direction: e.target.value as FlowlineDirection })}
          className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground"
        >
          <option value="u">U</option>
          <option value="v">V</option>
        </select>
      </FormField>
      <FormField label="Stepover (mm)" htmlFor="ffe-stepover">
        <Input id="ffe-stepover" type="number" min="0.001" step="0.01" defaultValue={params.stepover}
          onBlur={(e) => onSave({ stepover: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Allowance (mm)" htmlFor="ffe-allowance">
        <Input id="ffe-allowance" type="number" min="0" step="0.01" defaultValue={params.allowance}
          onBlur={(e) => onSave({ allowance: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Tool diameter (mm)" htmlFor="ffe-tool-diameter">
        <Input id="ffe-tool-diameter" type="number" min="0.1" step="0.1" defaultValue={params.toolDiameter}
          onBlur={(e) => onSave({ toolDiameter: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-in radius (mm)" htmlFor="ffe-arc-lead-in">
        <Input id="ffe-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-out radius (mm)" htmlFor="ffe-arc-lead-out">
        <Input id="ffe-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry radius (mm)" htmlFor="ffe-helical-radius">
        <Input id="ffe-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
          onBlur={(e) => onSave({ helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry pitch (mm)" htmlFor="ffe-helical-pitch">
        <Input id="ffe-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
          onBlur={(e) => onSave({ helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Ramp entry angle (deg)" htmlFor="ffe-ramp-angle">
        <Input id="ffe-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
          onBlur={(e) => onSave({ rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FaceSelectionBlock
        geometry={params.geometry}
        onSave={(geometry) => onSave({ geometry })}
      />
    </>
  )
}
