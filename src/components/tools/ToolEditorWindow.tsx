/**
 * ToolEditorWindow — root component for the tool editor window.
 *
 * Provides a context selector (Global Library / Project Tools tabs),
 * a filterable tool list, inline delete, and add/edit forms for both
 * global and project tool contexts.
 */

import { useState, useEffect, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { Plus, Download } from 'lucide-react'
import { useProjectStore, usePushNotification } from '../../store/projectStore'
import { useGlobalToolStore, useGlobalTools } from '../../store/globalToolStore'
import { listTools, deleteTool, addTool, editTool } from '../../api/tools'
import { deleteGlobalTool, listGlobalTools, addGlobalTool, editGlobalTool, importFromLibrary, exportToLibrary } from '../../api/globalTools'
import { toAppError } from '../../api/errors'
import { Button } from '@/components/ui/button'
import { ToolEditorList } from './ToolEditorList'
import { ToolEditorForm } from './ToolEditorForm'
import { ImportFromLibraryPicker } from './ImportFromLibraryPicker'
import type { Tool, ToolInput, ProjectSnapshot } from '../../api/types'

type ActiveContext = 'global' | 'project'
type EditorView = { tag: 'list' } | { tag: 'add' } | { tag: 'edit'; toolId: string } | { tag: 'import' }

export function ToolEditorWindow() {
  const [activeContext, setActiveContext] = useState<ActiveContext>('global')
  const [projectTools, setProjectTools] = useState<Tool[]>([])
  const [view, setView] = useState<EditorView>({ tag: 'list' })

  const snapshot = useProjectStore((s) => s.snapshot)
  const projectIsOpen = snapshot?.projectIsOpen ?? false
  const globalTools = useGlobalTools()
  const pushNotification = usePushNotification()
  const setGlobalTools = useGlobalToolStore((s) => s.setGlobalTools)

  // Fetch project tools when switching to project context or when snapshot changes
  const fetchProjectTools = useCallback(async () => {
    if (!projectIsOpen) return
    try {
      const tools = await listTools()
      setProjectTools(tools)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to load project tools')
    }
  }, [projectIsOpen, pushNotification])

  // Fetch project tools on context activation
  useEffect(() => {
    if (activeContext === 'project' && projectIsOpen) {
      void fetchProjectTools()
    }
  }, [activeContext, projectIsOpen, fetchProjectTools])

  // Auto-switch to global context when the project closes
  useEffect(() => {
    if (!projectIsOpen && activeContext === 'project') {
      setActiveContext('global')
      setProjectTools([])
      setView({ tag: 'list' })
    }
  }, [projectIsOpen, activeContext])

  // Listen for project:modified events — always re-fetch full project tools
  // since the event payload only carries ToolSummary[], but the editor needs
  // full Tool[] for the list and form.
  useEffect(() => {
    const unlistenPromise = listen<ProjectSnapshot>('project:modified', (event) => {
      if (event.payload.projectIsOpen) {
        void fetchProjectTools()
      }
    })
    return () => { void unlistenPromise.then((fn) => fn()) }
  }, [fetchProjectTools])

  function handleTabClick(ctx: ActiveContext) {
    if (ctx === 'project' && !projectIsOpen) return
    setView({ tag: 'list' })
    setActiveContext(ctx)
  }

  async function handleDelete(id: string) {
    try {
      if (activeContext === 'global') {
        await deleteGlobalTool(id)
        const refreshed = await listGlobalTools()
        setGlobalTools(refreshed)
      } else {
        await deleteTool(id)
        await fetchProjectTools()
      }
      if (view.tag === 'edit' && view.toolId === id) setView({ tag: 'list' })
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Delete failed')
    }
  }

  function handleEdit(id: string) {
    setView({ tag: 'edit', toolId: id })
  }

  async function handleFormSubmit(input: ToolInput, editId?: string) {
    try {
      if (editId) {
        if (activeContext === 'global') {
          await editGlobalTool(editId, input)
        } else {
          await editTool(editId, input)
        }
      } else {
        if (activeContext === 'global') {
          await addGlobalTool(input)
        } else {
          await addTool(input)
        }
      }
      // Refresh tool list
      if (activeContext === 'global') {
        const refreshed = await listGlobalTools()
        setGlobalTools(refreshed)
      } else {
        await fetchProjectTools()
      }
      setView({ tag: 'list' })
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Save failed')
    }
  }

  async function handleImport(ids: string[]) {
    try {
      await Promise.all(ids.map((id) => importFromLibrary(id)))
      await fetchProjectTools()
      pushNotification(`Imported ${ids.length} tool${ids.length > 1 ? 's' : ''} from library`)
      setView({ tag: 'list' })
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Import failed')
    }
  }

  async function handleExport(id: string) {
    try {
      await exportToLibrary(id)
      const refreshed = await listGlobalTools()
      setGlobalTools(refreshed)
      const tool = projectTools.find((t) => t.id === id)
      pushNotification(`Exported "${tool?.name ?? id}" to global library`)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Export failed')
    }
  }

  const tools = activeContext === 'global' ? globalTools : projectTools
  const editingTool = view.tag === 'edit'
    ? tools.find((t) => t.id === view.toolId)
    : undefined

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header
        data-testid="tool-editor-header"
        className="flex h-10 shrink-0 items-center border-b border-border"
      >
        <button
          role="tab"
          aria-selected={activeContext === 'global'}
          className={`h-full px-4 text-sm font-medium transition-colors ${
            activeContext === 'global'
              ? 'border-b-2 border-primary text-foreground'
              : 'text-muted-foreground hover:text-foreground'
          }`}
          onClick={() => handleTabClick('global')}
        >
          Global Library
        </button>
        <button
          role="tab"
          aria-selected={activeContext === 'project'}
          aria-disabled={!projectIsOpen || undefined}
          className={`h-full px-4 text-sm font-medium transition-colors ${
            activeContext === 'project'
              ? 'border-b-2 border-primary text-foreground'
              : !projectIsOpen
                ? 'cursor-not-allowed text-muted-foreground opacity-50'
                : 'text-muted-foreground hover:text-foreground'
          }`}
          onClick={() => handleTabClick('project')}
        >
          Project Tools
        </button>
      </header>

      <main
        data-testid="tool-editor-content"
        className="flex flex-1 overflow-hidden"
      >
        {/* Tool list sidebar */}
        <div className="flex w-72 shrink-0 flex-col border-r border-border p-2">
          {!projectIsOpen && activeContext === 'global' && (
            <p className="mb-2 rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">
              Open a project to manage project tools.
            </p>
          )}

          <ToolEditorList
            tools={tools}
            onEdit={handleEdit}
            onDelete={(id) => void handleDelete(id)}
            onExport={activeContext === 'project' ? (id) => void handleExport(id) : undefined}
          />

          <Button
            variant="outline"
            size="sm"
            className="mt-2 w-full"
            onClick={() => setView({ tag: 'add' })}
          >
            <Plus className="mr-1 h-3.5 w-3.5" />
            Add Tool
          </Button>

          {activeContext === 'project' && (
            <Button
              variant="outline"
              size="sm"
              className="mt-1 w-full"
              onClick={() => setView({ tag: 'import' })}
            >
              <Download className="mr-1 h-3.5 w-3.5" />
              Import from Library
            </Button>
          )}
        </div>

        {/* Edit area */}
        <div className="flex-1 overflow-auto p-4">
          {view.tag === 'import' ? (
            <ImportFromLibraryPicker
              tools={globalTools}
              onImport={(ids) => void handleImport(ids)}
              onCancel={() => setView({ tag: 'list' })}
            />
          ) : view.tag === 'add' ? (
            <ToolEditorForm
              onSubmit={(input) => handleFormSubmit(input)}
              onCancel={() => setView({ tag: 'list' })}
            />
          ) : view.tag === 'edit' && editingTool ? (
            <ToolEditorForm
              key={editingTool.id}
              initialTool={editingTool}
              onSubmit={(input) => handleFormSubmit(input, editingTool.id)}
              onCancel={() => setView({ tag: 'list' })}
            />
          ) : (
            <p className="text-sm text-muted-foreground">
              Select a tool to edit, or create a new one.
            </p>
          )}
        </div>
      </main>
    </div>
  )
}
