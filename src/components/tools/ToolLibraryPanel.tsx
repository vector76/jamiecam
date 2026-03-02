/**
 * ToolLibraryPanel — sidebar panel for managing the project tool library.
 *
 * Displays the list of tools from the project snapshot.  Each row shows the
 * tool name and type, with Edit and Delete buttons.  An "Add Tool" button
 * switches to an add form.  The edit button fetches full tool data from the
 * backend before switching to the edit form.
 */

import { useState } from 'react'
import { useTools, usePushNotification, useProjectStore } from '../../store/projectStore'
import { addTool, editTool, deleteTool, listTools } from '../../api/tools'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { Tool, ToolInput } from '../../api/types'

type Mode = { tag: 'list' } | { tag: 'add' } | { tag: 'edit'; tool: Tool }

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
]

export function ToolLibraryPanel() {
  const tools = useTools()
  const pushNotification = usePushNotification()
  const setSnapshot = useProjectStore((s) => s.setSnapshot)

  const [mode, setMode] = useState<Mode>({ tag: 'list' })

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  async function handleDelete(id: string) {
    try {
      await deleteTool(id)
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) { handleError(e) }
  }

  async function handleEditClick(id: string) {
    try {
      const all = await listTools()
      const tool = all.find((t) => t.id === id)
      if (!tool) return
      setMode({ tag: 'edit', tool })
    } catch (e) { handleError(e) }
  }

  if (mode.tag === 'add' || mode.tag === 'edit') {
    const isEdit = mode.tag === 'edit'
    const existing: Tool | undefined = isEdit ? mode.tool : undefined
    return (
      <ToolForm
        initial={existing}
        onSubmit={async (input) => {
          try {
            if (isEdit && existing) {
              await editTool(existing.id, input)
            } else {
              await addTool(input)
            }
            const snap = await getProjectSnapshot()
            setSnapshot(snap)
            setMode({ tag: 'list' })
          } catch (e) { handleError(e) }
        }}
        onCancel={() => setMode({ tag: 'list' })}
      />
    )
  }

  return (
    <div>
      {tools.map((t) => (
        <div key={t.id} style={{ display: 'flex', alignItems: 'center', gap: '0.25rem', marginBottom: '0.25rem' }}>
          <span style={{ flex: 1 }}>{t.name}</span>
          <span style={{ fontSize: '0.75em', color: '#666' }}>{t.toolType}</span>
          <button
            onClick={() => void handleEditClick(t.id)}
            aria-label={`Edit ${t.name}`}
          >
            Edit
          </button>
          <button
            onClick={() => void handleDelete(t.id)}
            aria-label={`Delete ${t.name}`}
          >
            ✕
          </button>
        </div>
      ))}
      <button onClick={() => setMode({ tag: 'add' })}>Add Tool</button>
    </div>
  )
}

interface ToolFormProps {
  initial?: Tool
  onSubmit: (input: ToolInput) => Promise<void>
  onCancel: () => void
}

function ToolForm({ initial, onSubmit, onCancel }: ToolFormProps) {
  const [name, setName] = useState(initial?.name ?? '')
  const [type, setType] = useState(initial?.type ?? TOOL_TYPES[0])
  const [material, setMaterial] = useState(initial?.material ?? '')
  const [diameter, setDiameter] = useState(initial?.diameter?.toString() ?? '')
  const [fluteCount, setFluteCount] = useState(initial?.fluteCount?.toString() ?? '')
  const [spindleSpeed, setSpindleSpeed] = useState(initial?.defaultSpindleSpeed?.toString() ?? '')
  const [feedRate, setFeedRate] = useState(initial?.defaultFeedRate?.toString() ?? '')

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    const spindleVal = parseFloat(spindleSpeed)
    const feedVal = parseFloat(feedRate)
    const input: ToolInput = {
      name,
      type,
      material,
      diameter: parseFloat(diameter),
      fluteCount: parseInt(fluteCount, 10),
      ...(isFinite(spindleVal) ? { defaultSpindleSpeed: spindleVal } : {}),
      ...(isFinite(feedVal) ? { defaultFeedRate: feedVal } : {}),
    }
    await onSubmit(input)
  }

  return (
    <form onSubmit={(e) => void handleSubmit(e)}>
      <div>
        <label>
          Name
          <input type="text" value={name} onChange={(e) => setName(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Type
          <select value={type} onChange={(e) => setType(e.target.value)}>
            {TOOL_TYPES.map((t) => (
              <option key={t} value={t}>{t}</option>
            ))}
          </select>
        </label>
      </div>
      <div>
        <label>
          Material
          <input type="text" value={material} onChange={(e) => setMaterial(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Diameter
          <input type="number" value={diameter} onChange={(e) => setDiameter(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Flute Count
          <input type="number" value={fluteCount} onChange={(e) => setFluteCount(e.target.value)} required />
        </label>
      </div>
      <div>
        <label>
          Default Spindle Speed
          <input type="number" value={spindleSpeed} onChange={(e) => setSpindleSpeed(e.target.value)} />
        </label>
      </div>
      <div>
        <label>
          Default Feed Rate
          <input type="number" value={feedRate} onChange={(e) => setFeedRate(e.target.value)} />
        </label>
      </div>
      <button type="submit">{initial ? 'Save' : 'Add'}</button>
      <button type="button" onClick={onCancel}>Cancel</button>
    </form>
  )
}
