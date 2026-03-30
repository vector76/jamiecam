/**
 * ImportFromLibraryPicker — displays global library tools with checkboxes
 * and an "Import Selected" button. Used inline in the project tools view
 * to import tools from the global library into the active project.
 */

import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import type { Tool } from '../../api/types'

/** Convert snake_case tool type to Title Case. */
function formatToolType(type: string): string {
  return type
    .split('_')
    .map((w) => w.charAt(0).toUpperCase() + w.slice(1))
    .join(' ')
}

interface ImportFromLibraryPickerProps {
  tools: Tool[]
  onImport: (ids: string[]) => void
  onCancel: () => void
}

export function ImportFromLibraryPicker({ tools, onImport, onCancel }: ImportFromLibraryPickerProps) {
  const [selected, setSelected] = useState<Set<string>>(new Set())

  function toggle(id: string) {
    setSelected((prev) => {
      const next = new Set(prev)
      if (next.has(id)) {
        next.delete(id)
      } else {
        next.add(id)
      }
      return next
    })
  }

  return (
    <div className="flex flex-col gap-2">
      <h3 className="text-xs font-semibold uppercase text-muted-foreground">
        Import from Global Library
      </h3>

      {tools.length === 0 ? (
        <p className="py-4 text-center text-sm text-muted-foreground">
          No tools in the global library.
        </p>
      ) : (
        <div className="max-h-60 space-y-0.5 overflow-auto">
          {tools.map((tool) => (
            <label
              key={tool.id}
              className="flex cursor-pointer items-center gap-2 rounded-sm px-1 py-0.5 hover:bg-accent"
            >
              <Checkbox
                checked={selected.has(tool.id)}
                onCheckedChange={() => toggle(tool.id)}
              />
              <span className="flex-1 truncate text-sm">{tool.name}</span>
              <span className="shrink-0 text-xs text-muted-foreground">
                {formatToolType(tool.type)}
              </span>
              <span className="shrink-0 text-xs text-muted-foreground">
                ⌀{tool.diameter} mm
              </span>
            </label>
          ))}
        </div>
      )}

      <div className="flex gap-2 pt-1">
        <Button
          size="sm"
          disabled={selected.size === 0}
          onClick={() => onImport(Array.from(selected))}
        >
          Import Selected
        </Button>
        <Button variant="ghost" size="sm" onClick={onCancel}>
          Cancel
        </Button>
      </div>
    </div>
  )
}
