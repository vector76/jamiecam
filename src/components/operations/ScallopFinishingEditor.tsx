/**
 * ScallopFinishingEditor — form fields for the Scallop Finishing operation.
 *
 * Receives params and an onSave callback that accepts a partial update.
 * The parent merges the partial into the full params before persisting.
 * Face selection is handled internally via the viewport store.
 */

import { Input } from '@/components/ui/input'
import { FormField } from '@/components/ui/form-field'
import { FaceSelectionBlock } from './FaceSelectionBlock'
import type { ScallopFinishingParams } from '../../api/types'

interface Props {
  params: ScallopFinishingParams
  onSave: (params: Partial<ScallopFinishingParams>) => void
}

export default function ScallopFinishingEditor({ params, onSave }: Props) {
  return (
    <>
      <FormField label="Target scallop height (mm)" htmlFor="sfe-scallop-height">
        <Input id="sfe-scallop-height" type="number" min="0.001" step="0.001" defaultValue={params.targetScallopHeight}
          onBlur={(e) => onSave({ targetScallopHeight: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Min stepover (mm)" htmlFor="sfe-min-stepover">
        <Input id="sfe-min-stepover" type="number" min="0.01" step="0.01" defaultValue={params.minStepover}
          onBlur={(e) => onSave({ minStepover: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Max stepover (mm)" htmlFor="sfe-max-stepover">
        <Input id="sfe-max-stepover" type="number" min="0.01" step="0.1" defaultValue={params.maxStepover}
          onBlur={(e) => onSave({ maxStepover: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Direction (deg)" htmlFor="sfe-direction">
        <Input id="sfe-direction" type="number" min="0" max="360" step="1" defaultValue={params.directionAngleDeg}
          onBlur={(e) => onSave({ directionAngleDeg: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Allowance (mm)" htmlFor="sfe-allowance">
        <Input id="sfe-allowance" type="number" min="0" step="0.01" defaultValue={params.allowance}
          onBlur={(e) => onSave({ allowance: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-in radius (mm)" htmlFor="sfe-arc-lead-in">
        <Input id="sfe-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-out radius (mm)" htmlFor="sfe-arc-lead-out">
        <Input id="sfe-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry radius (mm)" htmlFor="sfe-helical-radius">
        <Input id="sfe-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
          onBlur={(e) => onSave({ helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry pitch (mm)" htmlFor="sfe-helical-pitch">
        <Input id="sfe-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
          onBlur={(e) => onSave({ helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Ramp entry angle (deg)" htmlFor="sfe-ramp-angle">
        <Input id="sfe-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
          onBlur={(e) => onSave({ rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FaceSelectionBlock
        geometry={params.geometry}
        onSave={(geometry) => onSave({ geometry })}
      />
    </>
  )
}
