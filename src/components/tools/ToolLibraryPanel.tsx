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
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/ui/form-field'
import { Pencil, Trash2, Plus } from 'lucide-react'
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
    } catch (e) {
      handleError(e)
    }
  }

  async function handleEditClick(id: string) {
    try {
      const all = await listTools()
      const tool = all.find((t) => t.id === id)
      if (!tool) return
      setMode({ tag: 'edit', tool })
    } catch (e) {
      handleError(e)
    }
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
          } catch (e) {
            handleError(e)
          }
        }}
        onCancel={() => setMode({ tag: 'list' })}
      />
    )
  }

  return (
    <div className="space-y-1">
      {tools.map((t) => (
        <div
          key={t.id}
          className="flex items-center gap-1.5 rounded-sm px-1 py-0.5 hover:bg-accent"
        >
          <span className="flex-1 truncate text-sm" title={t.name}>{t.name}</span>
          <span className="text-xs text-muted-foreground">{t.toolType}</span>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6"
            onClick={() => void handleEditClick(t.id)}
            aria-label={`Edit ${t.name}`}
          >
            <Pencil className="h-3 w-3" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-6 w-6 text-muted-foreground hover:text-destructive"
            onClick={() => void handleDelete(t.id)}
            aria-label={`Delete ${t.name}`}
          >
            <Trash2 className="h-3 w-3" />
          </Button>
        </div>
      ))}
      <Button variant="outline" size="sm" onClick={() => setMode({ tag: 'add' })} className="w-full">
        <Plus className="mr-1 h-3.5 w-3.5" />
        Add Tool
      </Button>
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
    <form onSubmit={(e) => void handleSubmit(e)} className="space-y-0.5">
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
          value={type}
          onChange={(e) => setType(e.target.value)}
          className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground"
        >
          {TOOL_TYPES.map((t) => (
            <option key={t} value={t}>
              {t}
            </option>
          ))}
        </select>
      </FormField>
      <FormField label="Material" htmlFor="tool-material">
        <Input
          id="tool-material"
          type="text"
          value={material}
          onChange={(e) => setMaterial(e.target.value)}
          required
          className="h-7 text-xs"
        />
      </FormField>
      <FormField label="Diameter (mm)" htmlFor="tool-diameter">
        <Input
          id="tool-diameter"
          type="number"
          value={diameter}
          onChange={(e) => setDiameter(e.target.value)}
          required
          className="h-7 text-xs"
        />
      </FormField>
      <FormField label="Flute count" htmlFor="tool-flutes">
        <Input
          id="tool-flutes"
          type="number"
          value={fluteCount}
          onChange={(e) => setFluteCount(e.target.value)}
          required
          className="h-7 text-xs"
        />
      </FormField>
      <FormField label="Spindle speed (RPM)" htmlFor="tool-spindle">
        <Input
          id="tool-spindle"
          type="number"
          value={spindleSpeed}
          onChange={(e) => setSpindleSpeed(e.target.value)}
          className="h-7 text-xs"
        />
      </FormField>
      <FormField label="Feed rate (mm/min)" htmlFor="tool-feed">
        <Input
          id="tool-feed"
          type="number"
          value={feedRate}
          onChange={(e) => setFeedRate(e.target.value)}
          className="h-7 text-xs"
        />
      </FormField>
      <div className="flex gap-2 pt-2">
        <Button type="submit" size="sm">
          {initial ? 'Save' : 'Add'}
        </Button>
        <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </form>
  )
}
