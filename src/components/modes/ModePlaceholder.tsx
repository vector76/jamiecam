import { MODES } from './modeConfig'
import { checkUnsavedChanges } from '../../lib/unsavedGuard'
import { useProjectStore } from '../../store/projectStore'

interface ModePlaceholderProps {
  mode: string
}

export function ModePlaceholder({ mode }: ModePlaceholderProps) {
  const entry = MODES.find((m) => m.id === mode)

  async function handleBack() {
    const safe = await checkUnsavedChanges()
    if (!safe) return
    useProjectStore.getState().returnToSelector()
  }

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-background gap-4">
      {entry && (
        <h1 className="text-2xl font-semibold text-foreground">
          Mode {entry.number} — {entry.label}
        </h1>
      )}
      <p className="text-muted-foreground">Not yet implemented</p>
      <button type="button" onClick={handleBack}>
        Back
      </button>
    </div>
  )
}
