/**
 * ToolEditorWindow — root component for the tool editor window.
 *
 * Renders a placeholder layout with a header area (for context tabs)
 * and a main content area. Uses the same Tailwind base styles as AppShell.
 */

export function ToolEditorWindow() {
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header
        data-testid="tool-editor-header"
        className="flex h-10 shrink-0 items-center border-b border-border px-4"
      >
        <span className="text-sm font-medium">Tool Editor</span>
      </header>
      <main
        data-testid="tool-editor-content"
        className="flex-1 overflow-auto p-4"
      >
        <p className="text-sm text-muted-foreground">
          Select a tool to edit, or create a new one.
        </p>
      </main>
    </div>
  )
}
