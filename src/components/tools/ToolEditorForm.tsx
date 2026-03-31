/**
 * ToolEditorForm — comprehensive tool form with type-aware conditional
 * display of geometry parameters.
 *
 * Accepts an optional initialTool for editing (pre-populates all fields)
 * and an onSubmit callback that receives a ToolInput.
 */

import { useState, useId } from 'react'
import { ChevronDown } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/ui/form-field'
import { cn } from '@/lib/utils'
import type { Tool, ToolInput } from '../../api/types'

// ── Constants ─────────────────────────────────────────────────────────────────

const TOOL_TYPES = [
  'flat_endmill',
  'ball_nose',
  'bull_nose',
  'v_bit',
  'drill',
  'center_drill',
  'tap',
  'reamer',
  'boring_bar',
  'thread_mill',
] as const

const TOOL_TYPE_LABELS: Record<string, string> = {
  flat_endmill: 'Flat Endmill',
  ball_nose: 'Ball Nose',
  bull_nose: 'Bull Nose',
  v_bit: 'V-Bit',
  drill: 'Drill',
  center_drill: 'Center Drill',
  tap: 'Tap',
  reamer: 'Reamer',
  boring_bar: 'Boring Bar',
  thread_mill: 'Thread Mill',
}

/** Which type-specific fields are relevant per tool type. */
const TYPE_SPECIFIC_FIELDS: Record<string, string[]> = {
  flat_endmill: ['taperHalfAngle'],
  bull_nose: ['taperHalfAngle', 'cornerRadius'],
  v_bit: ['includedAngle'],
  drill: ['pointAngle'],
  center_drill: ['pointAngle', 'pilotDiameter', 'pilotLength'],
  tap: ['threadPitch'],
  thread_mill: ['threadPitch'],
  boring_bar: ['minBoreDiameter'],
  ball_nose: [],
  reamer: [],
}

// ── Component ─────────────────────────────────────────────────────────────────

interface ToolEditorFormProps {
  initialTool?: Tool
  onSubmit: (input: ToolInput) => Promise<void>
  onCancel?: () => void
}

export function ToolEditorForm({ initialTool, onSubmit, onCancel }: ToolEditorFormProps) {
  // Core fields
  const [name, setName] = useState(initialTool?.name ?? '')
  const [type, setType] = useState(initialTool?.type ?? 'flat_endmill')
  const [material, setMaterial] = useState(initialTool?.material ?? '')
  const [diameter, setDiameter] = useState(numStr(initialTool?.diameter))
  const [fluteCount, setFluteCount] = useState(numStr(initialTool?.fluteCount))
  const [spindleSpeed, setSpindleSpeed] = useState(numStr(initialTool?.defaultSpindleSpeed))
  const [feedRate, setFeedRate] = useState(numStr(initialTool?.defaultFeedRate))

  // Universal geometry (optional)
  const [cuttingLength, setCuttingLength] = useState(optGeomStr(initialTool?.cuttingLength))
  const [shankDiameter, setShankDiameter] = useState(optGeomStr(initialTool?.shankDiameter))
  const [overallLength, setOverallLength] = useState(optGeomStr(initialTool?.overallLength))

  // Type-specific geometry
  const [cornerRadius, setCornerRadius] = useState(numStr(initialTool?.cornerRadius))
  const [taperHalfAngle, setTaperHalfAngle] = useState(numStr(initialTool?.taperHalfAngle))
  const [includedAngle, setIncludedAngle] = useState(numStr(initialTool?.includedAngle))
  const [pointAngle, setPointAngle] = useState(numStr(initialTool?.pointAngle))
  const [pilotDiameter, setPilotDiameter] = useState(numStr(initialTool?.pilotDiameter))
  const [pilotLength, setPilotLength] = useState(numStr(initialTool?.pilotLength))
  const [threadPitch, setThreadPitch] = useState(numStr(initialTool?.threadPitch))
  const [minBoreDiameter, setMinBoreDiameter] = useState(numStr(initialTool?.minBoreDiameter))

  const [cuttingParamsOpen, setCuttingParamsOpen] = useState(false)
  const cuttingParamsId = useId()

  const activeSpecific = TYPE_SPECIFIC_FIELDS[type] ?? []

  function handleTypeChange(newType: string) {
    // Clear type-specific values that are not relevant to the new type
    const newFields = TYPE_SPECIFIC_FIELDS[newType] ?? []
    if (!newFields.includes('cornerRadius')) setCornerRadius('')
    if (!newFields.includes('taperHalfAngle')) setTaperHalfAngle('')
    if (!newFields.includes('includedAngle')) setIncludedAngle('')
    if (!newFields.includes('pointAngle')) setPointAngle('')
    if (!newFields.includes('pilotDiameter')) setPilotDiameter('')
    if (!newFields.includes('pilotLength')) setPilotLength('')
    if (!newFields.includes('threadPitch')) setThreadPitch('')
    if (!newFields.includes('minBoreDiameter')) setMinBoreDiameter('')
    setType(newType)
  }

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()

    const input: ToolInput = {
      name,
      type,
      diameter: parseFloat(diameter),
    }

    // Optional material and flute count
    const trimmedMaterial = material.trim()
    if (trimmedMaterial) input.material = trimmedMaterial

    const fc = parseInt(fluteCount, 10)
    if (!isNaN(fc)) input.fluteCount = fc

    // Optional common fields
    assignOpt(input, 'defaultSpindleSpeed', spindleSpeed)
    assignOpt(input, 'defaultFeedRate', feedRate)

    // Universal geometry (optional — backend applies defaults if omitted)
    assignOpt(input, 'cuttingLength', cuttingLength)
    assignOpt(input, 'shankDiameter', shankDiameter)
    assignOpt(input, 'overallLength', overallLength)

    // Type-specific geometry — only include fields relevant to current type
    if (activeSpecific.includes('cornerRadius')) assignOpt(input, 'cornerRadius', cornerRadius)
    if (activeSpecific.includes('taperHalfAngle')) assignOpt(input, 'taperHalfAngle', taperHalfAngle)
    if (activeSpecific.includes('includedAngle')) assignOpt(input, 'includedAngle', includedAngle)
    if (activeSpecific.includes('pointAngle')) assignOpt(input, 'pointAngle', pointAngle)
    if (activeSpecific.includes('pilotDiameter')) assignOpt(input, 'pilotDiameter', pilotDiameter)
    if (activeSpecific.includes('pilotLength')) assignOpt(input, 'pilotLength', pilotLength)
    if (activeSpecific.includes('threadPitch')) assignOpt(input, 'threadPitch', threadPitch)
    if (activeSpecific.includes('minBoreDiameter')) assignOpt(input, 'minBoreDiameter', minBoreDiameter)

    await onSubmit(input)
  }

  return (
    <form role="form" onSubmit={(e) => void handleSubmit(e)} className="space-y-3">
      {/* ── Group 1: Tool Identity (always visible) ───────────────────── */}
      <div className="space-y-1">
        <FormField label="Name" htmlFor="tool-name">
          <Input
            id="tool-name"
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            required
            className="h-7 text-xs"
          />
        </FormField>
        <FormField label="Type" htmlFor="tool-type">
          <select
            id="tool-type"
            aria-label="Type"
            value={type}
            onChange={(e) => handleTypeChange(e.target.value)}
            className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground"
          >
            {TOOL_TYPES.map((t) => (
              <option key={t} value={t}>
                {TOOL_TYPE_LABELS[t]}
              </option>
            ))}
          </select>
        </FormField>
      </div>

      {/* ── Group 2: Geometry (always visible) ───────────────────────── */}
      <section>
        <h3 className="mb-2 text-xs font-semibold uppercase text-muted-foreground">Geometry</h3>
        <div className="space-y-1">
          <FormField label="Diameter (mm)" htmlFor="tool-diameter">
            <Input
              id="tool-diameter"
              type="number"
              step="any"
              value={diameter}
              onChange={(e) => setDiameter(e.target.value)}
              required
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Cutting Length (mm)" htmlFor="tool-cutting-length">
            <Input
              id="tool-cutting-length"
              type="number"
              step="any"
              value={cuttingLength}
              onChange={(e) => setCuttingLength(e.target.value)}
              className="h-7 text-xs"
            />
          </FormField>
          {activeSpecific.includes('taperHalfAngle') && (
            <FormField label="Taper Half Angle (°)" htmlFor="tool-taper-half-angle">
              <Input
                id="tool-taper-half-angle"
                type="number"
                step="any"
                value={taperHalfAngle}
                onChange={(e) => setTaperHalfAngle(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('cornerRadius') && (
            <FormField label="Corner Radius (mm)" htmlFor="tool-corner-radius">
              <Input
                id="tool-corner-radius"
                type="number"
                step="any"
                value={cornerRadius}
                onChange={(e) => setCornerRadius(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('includedAngle') && (
            <FormField label="Included Angle (°)" htmlFor="tool-included-angle">
              <Input
                id="tool-included-angle"
                type="number"
                step="any"
                value={includedAngle}
                onChange={(e) => setIncludedAngle(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('pointAngle') && (
            <FormField label="Point Angle (°)" htmlFor="tool-point-angle">
              <Input
                id="tool-point-angle"
                type="number"
                step="any"
                value={pointAngle}
                onChange={(e) => setPointAngle(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('pilotDiameter') && (
            <FormField label="Pilot Diameter (mm)" htmlFor="tool-pilot-diameter">
              <Input
                id="tool-pilot-diameter"
                type="number"
                step="any"
                value={pilotDiameter}
                onChange={(e) => setPilotDiameter(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('pilotLength') && (
            <FormField label="Pilot Length (mm)" htmlFor="tool-pilot-length">
              <Input
                id="tool-pilot-length"
                type="number"
                step="any"
                value={pilotLength}
                onChange={(e) => setPilotLength(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('threadPitch') && (
            <FormField label="Thread Pitch (mm)" htmlFor="tool-thread-pitch">
              <Input
                id="tool-thread-pitch"
                type="number"
                step="any"
                value={threadPitch}
                onChange={(e) => setThreadPitch(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
          {activeSpecific.includes('minBoreDiameter') && (
            <FormField label="Min Bore Diameter (mm)" htmlFor="tool-min-bore-diameter">
              <Input
                id="tool-min-bore-diameter"
                type="number"
                step="any"
                value={minBoreDiameter}
                onChange={(e) => setMinBoreDiameter(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          )}
        </div>
      </section>

      {/* ── Group 3: Dimensions (always visible) ─────────────────────── */}
      <section>
        <h3 className="mb-2 text-xs font-semibold uppercase text-muted-foreground">Dimensions</h3>
        <div className="space-y-1">
          <FormField label="Shank Diameter (mm)" htmlFor="tool-shank-diameter">
            <Input
              id="tool-shank-diameter"
              type="number"
              step="any"
              value={shankDiameter}
              onChange={(e) => setShankDiameter(e.target.value)}
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Overall Length (mm)" htmlFor="tool-overall-length">
            <Input
              id="tool-overall-length"
              type="number"
              step="any"
              value={overallLength}
              onChange={(e) => setOverallLength(e.target.value)}
              className="h-7 text-xs"
            />
          </FormField>
        </div>
      </section>

      {/* ── Group 4: Cutting Parameters (collapsible) ─────────────────── */}
      <section>
        <button
          type="button"
          onClick={() => setCuttingParamsOpen(!cuttingParamsOpen)}
          aria-expanded={cuttingParamsOpen}
          aria-controls={cuttingParamsId}
          className="flex w-full items-center gap-1 px-3 py-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground hover:bg-accent"
        >
          <ChevronDown
            aria-hidden="true"
            className={cn('h-3.5 w-3.5 transition-transform', !cuttingParamsOpen && '-rotate-90')}
          />
          <span>Cutting Parameters</span>
        </button>
        {cuttingParamsOpen && (
          <div id={cuttingParamsId} className="space-y-1">
            <FormField label="Material" htmlFor="tool-material">
              <Input
                id="tool-material"
                type="text"
                value={material}
                onChange={(e) => setMaterial(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
            <FormField label="Flute Count" htmlFor="tool-flutes">
              <Input
                id="tool-flutes"
                type="number"
                step="1"
                value={fluteCount}
                onChange={(e) => setFluteCount(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
            <FormField label="Spindle Speed (RPM)" htmlFor="tool-spindle">
              <Input
                id="tool-spindle"
                type="number"
                step="any"
                value={spindleSpeed}
                onChange={(e) => setSpindleSpeed(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
            <FormField label="Feed Rate (mm/min)" htmlFor="tool-feed">
              <Input
                id="tool-feed"
                type="number"
                step="any"
                value={feedRate}
                onChange={(e) => setFeedRate(e.target.value)}
                className="h-7 text-xs"
              />
            </FormField>
          </div>
        )}
      </section>

      {/* ── Actions ───────────────────────────────────────────────────── */}
      <div className="flex gap-2 pt-2">
        <Button type="submit" size="sm">
          {initialTool ? 'Save' : 'Add'}
        </Button>
        {onCancel && (
          <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </Button>
        )}
      </div>
    </form>
  )
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Convert a number | undefined to a string for controlled inputs. */
function numStr(val: number | undefined): string {
  return val != null ? String(val) : ''
}

/**
 * Convert an optional geometry field from a Tool to a string for the form.
 * Returns '' for undefined so the input renders blank.
 */
function optGeomStr(val: number | undefined): string {
  return val != null ? String(val) : ''
}

/** If the string parses to a finite number, set it on the input object. */
function assignOpt(input: ToolInput, key: keyof ToolInput, raw: string) {
  const v = parseFloat(raw)
  if (isFinite(v)) {
    ;(input as unknown as Record<string, unknown>)[key] = v
  }
}
