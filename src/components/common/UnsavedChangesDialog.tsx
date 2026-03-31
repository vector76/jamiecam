/**
 * UnsavedChangesDialog — modal confirmation shown before destructive actions
 * when the project has unsaved changes.
 *
 * Reads `unsavedDialogOpen` from the project store. When open, presents three
 * choices: Save, Don't Save, and Cancel. Each resolves the pending promise
 * via `resolveUnsavedDialog`.
 */

import { useProjectStore } from '../../store/projectStore'
import { Button } from '@/components/ui/button'

export function UnsavedChangesDialog() {
  const open = useProjectStore((s) => s.unsavedDialogOpen)
  const resolve = useProjectStore((s) => s.resolveUnsavedDialog)

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div
        role="dialog"
        aria-modal="true"
        aria-label="Unsaved changes"
        className="w-full max-w-sm rounded-lg bg-card p-6 shadow-xl"
      >
        <p className="mb-6 text-sm text-card-foreground">
          You have unsaved changes. Save before continuing?
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={() => resolve('cancel')}>
            Cancel
          </Button>
          <Button variant="destructive" size="sm" onClick={() => resolve('discard')}>
            Don&apos;t Save
          </Button>
          <Button size="sm" onClick={() => resolve('save')}>
            Save
          </Button>
        </div>
      </div>
    </div>
  )
}
