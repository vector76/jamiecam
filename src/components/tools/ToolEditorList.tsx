/**
 * ToolEditorList — scrollable, filterable list of tools with edit/delete actions.
 *
 * Receives a Tool[] array and renders each row with name, formatted type,
 * diameter, and action buttons. A search input at the top filters by name.
 */

import { useState } from 'react'
import { Pencil, Trash2, Search, Upload } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import type { Tool } from '../../api/types'

/** Convert snake_case tool type to Title Case (e.g. "flat_endmill" → "Flat Endmill"). */
function formatToolType(type: string): string {
  return type
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

interface ToolEditorListProps {
  tools: Tool[]
  onEdit: (id: string) => void
  onDelete: (id: string) => void
  onExport?: (id: string) => void
}

export function ToolEditorList({ tools, onEdit, onDelete, onExport }: ToolEditorListProps) {
  const [search, setSearch] = useState('')

  const filtered = search
    ? tools.filter((t) => t.name.toLowerCase().includes(search.toLowerCase()))
    : tools

  return (
    <div className="flex flex-col gap-2">
      <div className="relative">
        <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
        <Input
          type="text"
          placeholder="Search tools…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-7 pl-7 text-xs"
        />
      </div>
      <div className="flex-1 overflow-auto">
        {filtered.length === 0 ? (
          <p className="py-4 text-center text-sm text-muted-foreground">No tools found.</p>
        ) : (
          <div className="space-y-0.5">
            {filtered.map((tool) => (
              <div
                key={tool.id}
                className="flex items-center gap-1.5 rounded-sm px-1 py-0.5 hover:bg-accent"
              >
                <span className="flex-1 truncate text-sm" title={tool.name}>
                  {tool.name}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  {formatToolType(tool.type)}
                </span>
                <span className="shrink-0 text-xs text-muted-foreground">
                  ⌀{tool.diameter} mm
                </span>
                {onExport && (
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6"
                    onClick={() => onExport(tool.id)}
                    aria-label={`Export ${tool.name}`}
                  >
                    <Upload className="h-3 w-3" />
                  </Button>
                )}
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6"
                  onClick={() => onEdit(tool.id)}
                  aria-label={`Edit ${tool.name}`}
                >
                  <Pencil className="h-3 w-3" />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-6 w-6 text-muted-foreground hover:text-destructive"
                  onClick={() => onDelete(tool.id)}
                  aria-label={`Delete ${tool.name}`}
                >
                  <Trash2 className="h-3 w-3" />
                </Button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}
