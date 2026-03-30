/**
 * Tests for ImportFromLibraryPicker — displays global library tools with
 * checkboxes and an "Import Selected" button that calls onImport with
 * the selected tool IDs.
 */

import { render, screen, fireEvent } from '@testing-library/react'
import { ImportFromLibraryPicker } from './ImportFromLibraryPicker'
import type { Tool } from '../../api/types'

const GLOBAL_TOOLS: Tool[] = [
  {
    id: 'gt-1',
    name: '8mm Global Flat',
    type: 'flat_endmill',
    material: 'Carbide',
    diameter: 8,
    fluteCount: 4,
    cuttingLength: 20,
    shankDiameter: 8,
    overallLength: 60,
  },
  {
    id: 'gt-2',
    name: '4mm Global Ball',
    type: 'ball_nose',
    material: 'HSS',
    diameter: 4,
    fluteCount: 2,
    cuttingLength: 12,
    shankDiameter: 4,
    overallLength: 40,
  },
  {
    id: 'gt-3',
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

describe('ImportFromLibraryPicker — rendering', () => {
  it('renders all global library tools', () => {
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={vi.fn()} onCancel={vi.fn()} />,
    )
    expect(screen.getByText('8mm Global Flat')).toBeInTheDocument()
    expect(screen.getByText('4mm Global Ball')).toBeInTheDocument()
    expect(screen.getByText('10mm Bull Nose')).toBeInTheDocument()
  })

  it('renders a checkbox for each tool', () => {
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={vi.fn()} onCancel={vi.fn()} />,
    )
    const checkboxes = screen.getAllByRole('checkbox')
    expect(checkboxes).toHaveLength(3)
  })

  it('renders empty message when no tools available', () => {
    render(
      <ImportFromLibraryPicker tools={[]} onImport={vi.fn()} onCancel={vi.fn()} />,
    )
    expect(screen.getByText(/no tools in the global library/i)).toBeInTheDocument()
  })

  it('disables import button when nothing is selected', () => {
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={vi.fn()} onCancel={vi.fn()} />,
    )
    expect(screen.getByRole('button', { name: /import selected/i })).toBeDisabled()
  })
})

describe('ImportFromLibraryPicker — selection and import', () => {
  it('enables import button when at least one tool is checked', () => {
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={vi.fn()} onCancel={vi.fn()} />,
    )
    fireEvent.click(screen.getAllByRole('checkbox')[0])
    expect(screen.getByRole('button', { name: /import selected/i })).toBeEnabled()
  })

  it('calls onImport with selected tool IDs', () => {
    const onImport = vi.fn()
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={onImport} onCancel={vi.fn()} />,
    )
    // Select first and third tools
    fireEvent.click(screen.getAllByRole('checkbox')[0])
    fireEvent.click(screen.getAllByRole('checkbox')[2])
    fireEvent.click(screen.getByRole('button', { name: /import selected/i }))

    expect(onImport).toHaveBeenCalledWith(['gt-1', 'gt-3'])
  })

  it('toggles selection off when clicking a checked checkbox', () => {
    const onImport = vi.fn()
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={onImport} onCancel={vi.fn()} />,
    )
    const firstCheckbox = screen.getAllByRole('checkbox')[0]
    fireEvent.click(firstCheckbox) // check
    fireEvent.click(firstCheckbox) // uncheck
    // Import button should be disabled again
    expect(screen.getByRole('button', { name: /import selected/i })).toBeDisabled()
  })

  it('calls onCancel when cancel is clicked', () => {
    const onCancel = vi.fn()
    render(
      <ImportFromLibraryPicker tools={GLOBAL_TOOLS} onImport={vi.fn()} onCancel={onCancel} />,
    )
    fireEvent.click(screen.getByRole('button', { name: /cancel/i }))
    expect(onCancel).toHaveBeenCalled()
  })
})
