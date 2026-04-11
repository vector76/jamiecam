import type { Mode } from '../../api/types'

export interface ModeEntry {
  id: Mode
  number: number
  label: string
  description: string
}

export const MODES: ModeEntry[] = [
  { id: 'gcode_viewer', number: 1, label: 'G-code Viewer', description: 'Load and simulate G-code from any source' },
  { id: '2d',          number: 2, label: '2D',            description: 'Profile, pocket, and drill operations from SVG / DXF artwork' },
  { id: '2_5d',        number: 3, label: '2.5D',          description: 'V-carve and relief operations from 2D artwork with 3D toolpaths' },
  { id: '3d',          number: 4, label: '3D Surface',    description: '3-axis surface finishing from heightmaps, meshes, or solid models' },
  { id: 'rotary_2',    number: 5, label: '2 + Rotary',    description: '2 linear axes plus one rotary (X, Z, A) for cylindrical work' },
  { id: 'rotary_3',    number: 6, label: '3 + Rotary',    description: '3 linear axes plus one rotary (X, Y, Z, A) for 4-axis simultaneous' },
  { id: '5_axis',      number: 7, label: '5-Axis',        description: 'Full 5-axis simultaneous machining (3 linear + 2 rotary)' },
]
