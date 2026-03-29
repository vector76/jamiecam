/**
 * ParallelFinishingEditor — form fields for the Parallel Finishing operation.
 *
 * Receives params and an onSave callback that accepts a partial update.
 * The parent merges the partial into the full params before persisting.
 * Face selection is handled internally via the viewport store.
 */

import { Input } from '@/components/ui/input'
import { FormField } from '@/components/ui/form-field'
import { FaceSelectionBlock } from './FaceSelectionBlock'
import type { ParallelFinishingParams } from '../../api/types'

interface Props {
  params: ParallelFinishingParams
  onSave: (params: Partial<ParallelFinishingParams>) => void
}

export default function ParallelFinishingEditor({ params, onSave }: Props) {
  return (
    <>
      <FormField label="Stepover (mm)" htmlFor="pfe-stepover">
        <Input id="pfe-stepover" type="number" min="0.01" step="0.1" defaultValue={params.stepover}
          onBlur={(e) => onSave({ stepover: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Direction (deg)" htmlFor="pfe-direction">
        <Input id="pfe-direction" type="number" min="0" max="360" step="1" defaultValue={params.directionAngleDeg}
          onBlur={(e) => onSave({ directionAngleDeg: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Allowance (mm)" htmlFor="pfe-allowance">
        <Input id="pfe-allowance" type="number" min="0" step="0.01" defaultValue={params.allowance}
          onBlur={(e) => onSave({ allowance: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-in radius (mm)" htmlFor="pfe-arc-lead-in">
        <Input id="pfe-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-out radius (mm)" htmlFor="pfe-arc-lead-out">
        <Input id="pfe-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry radius (mm)" htmlFor="pfe-helical-radius">
        <Input id="pfe-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
          onBlur={(e) => onSave({ helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry pitch (mm)" htmlFor="pfe-helical-pitch">
        <Input id="pfe-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
          onBlur={(e) => onSave({ helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Ramp entry angle (deg)" htmlFor="pfe-ramp-angle">
        <Input id="pfe-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
          onBlur={(e) => onSave({ rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FaceSelectionBlock
        geometry={params.geometry}
        onSave={(geometry) => onSave({ geometry })}
      />
    </>
  )
}
