/**
 * Tests for ToolEditorList.tsx — tool list display, search filtering,
 * edit/delete actions.
 */

import { render, screen, fireEvent } from '@testing-library/react'
import { ToolEditorList } from './ToolEditorList'
import type { Tool } from '../../api/types'

// ── Fixtures ──────────────────────────────────────────────────────────────────

const TOOLS: Tool[] = [
  {
    id: 'tool-1',
    name: '6mm Flat Endmill',
    type: 'flat_endmill',
    material: 'HSS',
    diameter: 6,
    fluteCount: 4,
    cuttingLength: 18,
    shankDiameter: 6,
    overallLength: 54,
  },
  {
    id: 'tool-2',
    name: '3mm Ball Nose',
    type: 'ball_nose',
    material: 'Carbide',
    diameter: 3,
    fluteCount: 2,
    cuttingLength: 12,
    shankDiameter: 3,
    overallLength: 40,
  },
  {
    id: 'tool-3',
    name: '10mm Bull Nose',
    type: 'bull_nose',
    material: 'Carbide',
    diameter: 10,
    fluteCount: 3,
    cornerRadius: 2,
    cuttingLength: 25,
    shankDiameter: 10,
    overallLength: 70,
  },
]

// ── Rendering ─────────────────────────────────────────────────────────────────

describe('ToolEditorList — rendering', () => {
  it('renders tool names, formatted types, and diameters', () => {
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={vi.fn()} />)
    expect(screen.getByText('6mm Flat Endmill')).toBeInTheDocument()
    expect(screen.getByText('3mm Ball Nose')).toBeInTheDocument()
    expect(screen.getByText('Flat Endmill')).toBeInTheDocument()
    expect(screen.getByText('Ball Nose')).toBeInTheDocument()
    expect(screen.getByText('⌀6 mm')).toBeInTheDocument()
    expect(screen.getByText('⌀3 mm')).toBeInTheDocument()
  })

  it('renders edit and delete buttons for each tool', () => {
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={vi.fn()} />)
    expect(screen.getByRole('button', { name: 'Edit 6mm Flat Endmill' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Delete 6mm Flat Endmill' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Edit 3mm Ball Nose' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Delete 3mm Ball Nose' })).toBeInTheDocument()
  })

  it('renders empty state when no tools', () => {
    render(<ToolEditorList tools={[]} onEdit={vi.fn()} onDelete={vi.fn()} />)
    expect(screen.getByText(/no tools/i)).toBeInTheDocument()
  })
})

// ── Search / filter ──────────────────────────────────────────────────────────

describe('ToolEditorList — search', () => {
  it('filters tools by name (case-insensitive)', () => {
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={vi.fn()} />)
    const searchInput = screen.getByPlaceholderText(/search/i)

    fireEvent.change(searchInput, { target: { value: 'ball' } })

    expect(screen.getByText('3mm Ball Nose')).toBeInTheDocument()
    expect(screen.queryByText('6mm Flat Endmill')).not.toBeInTheDocument()
    expect(screen.queryByText('10mm Bull Nose')).not.toBeInTheDocument()
  })

  it('shows all tools when search is cleared', () => {
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={vi.fn()} />)
    const searchInput = screen.getByPlaceholderText(/search/i)

    fireEvent.change(searchInput, { target: { value: 'ball' } })
    fireEvent.change(searchInput, { target: { value: '' } })

    expect(screen.getByText('6mm Flat Endmill')).toBeInTheDocument()
    expect(screen.getByText('3mm Ball Nose')).toBeInTheDocument()
    expect(screen.getByText('10mm Bull Nose')).toBeInTheDocument()
  })

  it('shows empty state when search has no matches', () => {
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={vi.fn()} />)
    const searchInput = screen.getByPlaceholderText(/search/i)

    fireEvent.change(searchInput, { target: { value: 'zzz' } })

    expect(screen.queryByText('6mm Flat Endmill')).not.toBeInTheDocument()
    expect(screen.getByText(/no tools/i)).toBeInTheDocument()
  })
})

// ── Actions ──────────────────────────────────────────────────────────────────

describe('ToolEditorList — actions', () => {
  it('calls onEdit with tool id when edit button is clicked', () => {
    const onEdit = vi.fn()
    render(<ToolEditorList tools={TOOLS} onEdit={onEdit} onDelete={vi.fn()} />)

    fireEvent.click(screen.getByRole('button', { name: 'Edit 3mm Ball Nose' }))
    expect(onEdit).toHaveBeenCalledWith('tool-2')
  })

  it('calls onDelete with tool id when delete button is clicked', () => {
    const onDelete = vi.fn()
    render(<ToolEditorList tools={TOOLS} onEdit={vi.fn()} onDelete={onDelete} />)

    fireEvent.click(screen.getByRole('button', { name: 'Delete 6mm Flat Endmill' }))
    expect(onDelete).toHaveBeenCalledWith('tool-1')
  })
})
