/**
 * ToolEditorWindow — root component for the tool editor window.
 *
 * Provides a context selector (Global Library / Project Tools tabs),
 * a filterable tool list, and inline delete. The actual edit form
 * will be built in a subsequent bead — for now, edit clicks show a placeholder.
 */

import { useState, useEffect, useCallback } from 'react'
import { listen } from '@tauri-apps/api/event'
import { useProjectStore, usePushNotification } from '../../store/projectStore'
import { useGlobalToolStore, useGlobalTools } from '../../store/globalToolStore'
import { listTools, deleteTool } from '../../api/tools'
import { deleteGlobalTool, listGlobalTools } from '../../api/globalTools'
import { toAppError } from '../../api/errors'
import { ToolEditorList } from './ToolEditorList'
import type { Tool, ProjectSnapshot } from '../../api/types'

type ActiveContext = 'global' | 'project'

export function ToolEditorWindow() {
  const [activeContext, setActiveContext] = useState<ActiveContext>('global')
  const [projectTools, setProjectTools] = useState<Tool[]>([])
  const [editingToolId, setEditingToolId] = useState<string | null>(null)

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
      setEditingToolId(null)
    }
  }, [projectIsOpen, activeContext])

  // Listen for project:modified events — refresh project tools if in project context
  useEffect(() => {
    const unlistenPromise = listen<ProjectSnapshot>('project:modified', () => {
      if (activeContext === 'project') {
        void fetchProjectTools()
      }
    })
    return () => { void unlistenPromise.then((fn) => fn()) }
  }, [activeContext, fetchProjectTools])

  function handleTabClick(ctx: ActiveContext) {
    if (ctx === 'project' && !projectIsOpen) return
    setEditingToolId(null)
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
      if (editingToolId === id) setEditingToolId(null)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Delete failed')
    }
  }

  function handleEdit(id: string) {
    setEditingToolId(id)
  }

  const tools = activeContext === 'global' ? globalTools : projectTools

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
          {!projectIsOpen && (
            <p className="mb-2 rounded-sm bg-muted px-2 py-1 text-xs text-muted-foreground">
              Open a project to manage project tools.
            </p>
          )}

          <ToolEditorList
            tools={tools}
            onEdit={handleEdit}
            onDelete={(id) => void handleDelete(id)}
          />
        </div>

        {/* Edit area */}
        <div className="flex-1 overflow-auto p-4">
          {editingToolId ? (
            <p className="text-sm text-muted-foreground">
              Editing tool: {editingToolId}
            </p>
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
