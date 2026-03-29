/**
 * PencilMillingEditor — form fields for the Pencil Milling operation.
 *
 * Receives params and an onSave callback that accepts a partial update.
 * The parent merges the partial into the full params before persisting.
 * Face selection is handled internally via the viewport store.
 *
 * The curvature threshold field uses a placeholder "Auto" when the value
 * is null/undefined. Clearing the field saves null (tool-radius-derived default).
 */

import { Input } from '@/components/ui/input'
import { FormField } from '@/components/ui/form-field'
import { FaceSelectionBlock } from './FaceSelectionBlock'
import type { PencilMillingParams } from '../../api/types'

interface Props {
  params: PencilMillingParams
  onSave: (params: Partial<PencilMillingParams>) => void
}

export default function PencilMillingEditor({ params, onSave }: Props) {
  return (
    <>
      <FormField label="Curvature threshold (mm)" htmlFor="pme-curvature-threshold">
        <Input id="pme-curvature-threshold" type="number" min="0" step="0.1"
          placeholder="Auto"
          defaultValue={params.curvatureThreshold ?? ''}
          onBlur={(e) => onSave({ curvatureThreshold: e.target.value === '' ? null : parseFloat(e.target.value) })}
          className="h-7 text-xs" />
      </FormField>
      <FormField label="Min pass length (mm)" htmlFor="pme-min-pass-length">
        <Input id="pme-min-pass-length" type="number" min="0" step="0.1" defaultValue={params.minPassLength}
          onBlur={(e) => onSave({ minPassLength: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Allowance (mm)" htmlFor="pme-allowance">
        <Input id="pme-allowance" type="number" min="0" step="0.01" defaultValue={params.allowance}
          onBlur={(e) => onSave({ allowance: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Tool diameter (mm)" htmlFor="pme-tool-diameter">
        <Input id="pme-tool-diameter" type="number" min="0.01" step="0.1" defaultValue={params.toolDiameter}
          onBlur={(e) => onSave({ toolDiameter: parseFloat(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-in radius (mm)" htmlFor="pme-arc-lead-in">
        <Input id="pme-arc-lead-in" type="number" defaultValue={params.arcLeadInRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadInRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Arc lead-out radius (mm)" htmlFor="pme-arc-lead-out">
        <Input id="pme-arc-lead-out" type="number" defaultValue={params.arcLeadOutRadius ?? ''}
          onBlur={(e) => onSave({ arcLeadOutRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry radius (mm)" htmlFor="pme-helical-radius">
        <Input id="pme-helical-radius" type="number" defaultValue={params.helicalEntryRadius ?? ''}
          onBlur={(e) => onSave({ helicalEntryRadius: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Helical entry pitch (mm)" htmlFor="pme-helical-pitch">
        <Input id="pme-helical-pitch" type="number" defaultValue={params.helicalEntryPitch ?? ''}
          onBlur={(e) => onSave({ helicalEntryPitch: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FormField label="Ramp entry angle (deg)" htmlFor="pme-ramp-angle">
        <Input id="pme-ramp-angle" type="number" defaultValue={params.rampEntryAngleDeg ?? ''}
          onBlur={(e) => onSave({ rampEntryAngleDeg: e.target.value === '' ? null : Number(e.target.value) })} className="h-7 text-xs" />
      </FormField>
      <FaceSelectionBlock
        geometry={params.geometry}
        onSave={(geometry) => onSave({ geometry })}
      />
    </>
  )
}
