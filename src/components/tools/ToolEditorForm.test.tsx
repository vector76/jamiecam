/**
 * Tests for ToolEditorForm — type-aware tool form with conditional geometry fields.
 */

import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { ToolEditorForm } from './ToolEditorForm'
import type { Tool, ToolInput } from '../../api/types'

// ── Fixtures ──────────────────────────────────────────────────────────────────

const FULL_TOOL: Tool = {
  id: 'aaaaaaaa-0000-0000-0000-000000000001',
  name: 'My Bull Nose',
  type: 'bull_nose',
  material: 'Carbide',
  diameter: 6,
  fluteCount: 4,
  defaultSpindleSpeed: 12000,
  defaultFeedRate: 800,
  cuttingLength: 18,
  shankDiameter: 6,
  overallLength: 54,
  cornerRadius: 1.5,
  taperHalfAngle: 2,
}

const V_BIT_TOOL: Tool = {
  id: 'aaaaaaaa-0000-0000-0000-000000000002',
  name: 'V-Cutter',
  type: 'v_bit',
  material: 'HSS',
  diameter: 3,
  fluteCount: 2,
  cuttingLength: 9,
  shankDiameter: 3,
  overallLength: 27,
  includedAngle: 90,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/** Labels for type-specific fields to check visibility. */
const TYPE_SPECIFIC_LABELS = {
  taperHalfAngle: /taper half angle/i,
  cornerRadius: /corner radius/i,
  includedAngle: /included angle/i,
  pointAngle: /point angle/i,
  pilotDiameter: /pilot diameter/i,
  pilotLength: /pilot length/i,
  threadPitch: /thread pitch/i,
  minBoreDiameter: /min bore diameter/i,
}

function selectToolType(value: string) {
  fireEvent.change(screen.getByLabelText(/^type$/i), { target: { value } })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('ToolEditorForm', () => {
  describe('type-specific field visibility', () => {
    it('shows taper half angle for flat_endmill', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('flat_endmill')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.taperHalfAngle)).toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.cornerRadius)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.includedAngle)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.pointAngle)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.threadPitch)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.minBoreDiameter)).not.toBeInTheDocument()
    })

    it('shows taper half angle and corner radius for bull_nose', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('bull_nose')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.taperHalfAngle)).toBeInTheDocument()
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.cornerRadius)).toBeInTheDocument()
    })

    it('shows included angle for v_bit', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('v_bit')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.includedAngle)).toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.taperHalfAngle)).not.toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.cornerRadius)).not.toBeInTheDocument()
    })

    it('shows point angle for drill', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('drill')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.pointAngle)).toBeInTheDocument()
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.pilotDiameter)).not.toBeInTheDocument()
    })

    it('shows point angle, pilot diameter, and pilot length for center_drill', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('center_drill')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.pointAngle)).toBeInTheDocument()
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.pilotDiameter)).toBeInTheDocument()
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.pilotLength)).toBeInTheDocument()
    })

    it('shows thread pitch for tap', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('tap')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.threadPitch)).toBeInTheDocument()
    })

    it('shows thread pitch for thread_mill', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('thread_mill')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.threadPitch)).toBeInTheDocument()
    })

    it('shows min bore diameter for boring_bar', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('boring_bar')

      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.minBoreDiameter)).toBeInTheDocument()
    })

    it('shows no type-specific fields for ball_nose', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('ball_nose')

      for (const label of Object.values(TYPE_SPECIFIC_LABELS)) {
        expect(screen.queryByLabelText(label)).not.toBeInTheDocument()
      }
    })

    it('shows no type-specific fields for reamer', () => {
      render(<ToolEditorForm onSubmit={vi.fn()} />)
      selectToolType('reamer')

      for (const label of Object.values(TYPE_SPECIFIC_LABELS)) {
        expect(screen.queryByLabelText(label)).not.toBeInTheDocument()
      }
    })

    it('clears type-specific values when switching types', async () => {
      const onSubmit = vi.fn<(input: ToolInput) => Promise<void>>().mockResolvedValue(undefined)
      render(<ToolEditorForm onSubmit={onSubmit} />)

      // Fill required fields
      fireEvent.change(screen.getByLabelText(/^name$/i), { target: { value: 'Test' } })
      fireEvent.change(screen.getByLabelText(/material/i), { target: { value: 'Carbide' } })
      fireEvent.change(screen.getByLabelText(/^diameter/i), { target: { value: '6' } })
      fireEvent.change(screen.getByLabelText(/flute count/i), { target: { value: '4' } })

      // Select v_bit and fill included angle
      selectToolType('v_bit')
      fireEvent.change(screen.getByLabelText(TYPE_SPECIFIC_LABELS.includedAngle), {
        target: { value: '90' },
      })

      // Switch to drill — included angle should be cleared
      selectToolType('drill')
      expect(screen.queryByLabelText(TYPE_SPECIFIC_LABELS.includedAngle)).not.toBeInTheDocument()

      // Submit and verify included angle is not in the output
      fireEvent.submit(screen.getByRole('form'))

      await waitFor(() => {
        expect(onSubmit).toHaveBeenCalledTimes(1)
      })

      const input = onSubmit.mock.calls[0][0]
      expect(input.includedAngle).toBeUndefined()
    })
  })

  describe('pre-populating edit form', () => {
    it('populates all fields from an existing tool', () => {
      render(<ToolEditorForm initialTool={FULL_TOOL} onSubmit={vi.fn()} />)

      expect(screen.getByLabelText(/^name$/i)).toHaveValue('My Bull Nose')
      expect(screen.getByLabelText(/^type$/i)).toHaveValue('bull_nose')
      expect(screen.getByLabelText(/material/i)).toHaveValue('Carbide')
      expect(screen.getByLabelText(/^diameter/i)).toHaveValue(6)
      expect(screen.getByLabelText(/flute count/i)).toHaveValue(4)
      expect(screen.getByLabelText(/spindle speed/i)).toHaveValue(12000)
      expect(screen.getByLabelText(/feed rate/i)).toHaveValue(800)
      expect(screen.getByLabelText(/cutting length/i)).toHaveValue(18)
      expect(screen.getByLabelText(/shank diameter/i)).toHaveValue(6)
      expect(screen.getByLabelText(/overall length/i)).toHaveValue(54)
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.cornerRadius)).toHaveValue(1.5)
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.taperHalfAngle)).toHaveValue(2)
    })

    it('populates v_bit fields from an existing tool', () => {
      render(<ToolEditorForm initialTool={V_BIT_TOOL} onSubmit={vi.fn()} />)

      expect(screen.getByLabelText(/^name$/i)).toHaveValue('V-Cutter')
      expect(screen.getByLabelText(/^type$/i)).toHaveValue('v_bit')
      expect(screen.getByLabelText(TYPE_SPECIFIC_LABELS.includedAngle)).toHaveValue(90)
    })
  })

  describe('submit assembles correct ToolInput', () => {
    it('omits blank optional number fields', async () => {
      const onSubmit = vi.fn<(input: ToolInput) => Promise<void>>().mockResolvedValue(undefined)
      render(<ToolEditorForm onSubmit={onSubmit} />)

      // Fill only required fields (type defaults to flat_endmill)
      fireEvent.change(screen.getByLabelText(/^name$/i), { target: { value: 'Basic EM' } })
      fireEvent.change(screen.getByLabelText(/material/i), { target: { value: 'HSS' } })
      fireEvent.change(screen.getByLabelText(/^diameter/i), { target: { value: '10' } })
      fireEvent.change(screen.getByLabelText(/flute count/i), { target: { value: '2' } })

      fireEvent.submit(screen.getByRole('form'))

      await waitFor(() => {
        expect(onSubmit).toHaveBeenCalledTimes(1)
      })

      const input = onSubmit.mock.calls[0][0]
      expect(input.name).toBe('Basic EM')
      expect(input.type).toBe('flat_endmill')
      expect(input.material).toBe('HSS')
      expect(input.diameter).toBe(10)
      expect(input.fluteCount).toBe(2)
      // All optional fields should be undefined (omitted)
      expect(input.defaultSpindleSpeed).toBeUndefined()
      expect(input.defaultFeedRate).toBeUndefined()
      expect(input.cuttingLength).toBeUndefined()
      expect(input.shankDiameter).toBeUndefined()
      expect(input.overallLength).toBeUndefined()
      expect(input.cornerRadius).toBeUndefined()
      expect(input.includedAngle).toBeUndefined()
      expect(input.taperHalfAngle).toBeUndefined()
    })

    it('includes filled optional fields in submit', async () => {
      const onSubmit = vi.fn<(input: ToolInput) => Promise<void>>().mockResolvedValue(undefined)
      render(<ToolEditorForm onSubmit={onSubmit} />)

      fireEvent.change(screen.getByLabelText(/^name$/i), { target: { value: 'Drill Bit' } })
      fireEvent.change(screen.getByLabelText(/material/i), { target: { value: 'Carbide' } })
      fireEvent.change(screen.getByLabelText(/^diameter/i), { target: { value: '5' } })
      fireEvent.change(screen.getByLabelText(/flute count/i), { target: { value: '2' } })
      selectToolType('drill')
      fireEvent.change(screen.getByLabelText(/spindle speed/i), { target: { value: '8000' } })
      fireEvent.change(screen.getByLabelText(/cutting length/i), { target: { value: '15' } })
      fireEvent.change(screen.getByLabelText(TYPE_SPECIFIC_LABELS.pointAngle), {
        target: { value: '118' },
      })

      fireEvent.submit(screen.getByRole('form'))

      await waitFor(() => {
        expect(onSubmit).toHaveBeenCalledTimes(1)
      })

      const input = onSubmit.mock.calls[0][0]
      expect(input.type).toBe('drill')
      expect(input.defaultSpindleSpeed).toBe(8000)
      expect(input.cuttingLength).toBe(15)
      expect(input.pointAngle).toBe(118)
      // Non-drill type-specific fields should be absent
      expect(input.cornerRadius).toBeUndefined()
      expect(input.includedAngle).toBeUndefined()
      expect(input.threadPitch).toBeUndefined()
    })

    it('submits full edit form correctly', async () => {
      const onSubmit = vi.fn<(input: ToolInput) => Promise<void>>().mockResolvedValue(undefined)
      render(<ToolEditorForm initialTool={FULL_TOOL} onSubmit={onSubmit} />)

      // Change the name
      fireEvent.change(screen.getByLabelText(/^name$/i), {
        target: { value: 'Updated Bull Nose' },
      })

      fireEvent.submit(screen.getByRole('form'))

      await waitFor(() => {
        expect(onSubmit).toHaveBeenCalledTimes(1)
      })

      const input = onSubmit.mock.calls[0][0]
      expect(input.name).toBe('Updated Bull Nose')
      expect(input.type).toBe('bull_nose')
      expect(input.cornerRadius).toBe(1.5)
      expect(input.taperHalfAngle).toBe(2)
      expect(input.defaultSpindleSpeed).toBe(12000)
      expect(input.defaultFeedRate).toBe(800)
      expect(input.cuttingLength).toBe(18)
      expect(input.shankDiameter).toBe(6)
      expect(input.overallLength).toBe(54)
      // Non-bull_nose fields should be absent
      expect(input.includedAngle).toBeUndefined()
      expect(input.pointAngle).toBeUndefined()
    })
  })

  describe('cancel', () => {
    it('calls onCancel when cancel button is clicked', () => {
      const onCancel = vi.fn()
      render(<ToolEditorForm onSubmit={vi.fn()} onCancel={onCancel} />)

      fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
      expect(onCancel).toHaveBeenCalledTimes(1)
    })
  })
})
