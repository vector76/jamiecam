/**
 * Toolbar — primary file operation buttons.
 *
 * Delegates to shared menu action handlers in lib/menuActions so the same
 * logic can be invoked from native menu event listeners.  Errors are
 * surfaced through the global Notifications system.
 */

import { Button } from '@/components/ui/button'
import { openToolEditor } from '../../api/window'
import { handleOpenModel, handleNewProject, handleSaveAs, handleOpenProject } from '../../lib/menuActions'
import { FolderOpen, FilePlus, Save, FolderInput, Wrench } from 'lucide-react'

// ── Component ─────────────────────────────────────────────────────────────────

export function Toolbar() {
  return (
    <div className="flex items-center gap-1.5 border-b border-border bg-card px-2 py-1">
      <Button variant="ghost" size="sm" onClick={() => void handleOpenModel()}>
        <FolderOpen className="mr-1 h-3.5 w-3.5" />
        Open Model
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleNewProject()}>
        <FilePlus className="mr-1 h-3.5 w-3.5" />
        New Project
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleSaveAs()}>
        <Save className="mr-1 h-3.5 w-3.5" />
        Save Project
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void handleOpenProject()}>
        <FolderInput className="mr-1 h-3.5 w-3.5" />
        Open Project
      </Button>
      <Button variant="ghost" size="sm" onClick={() => void openToolEditor()}>
        <Wrench className="mr-1 h-3.5 w-3.5" />
        Tool Editor
      </Button>
    </div>
  )
}
