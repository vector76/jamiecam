/**
 * GCodePreviewPanel — shows a G-code preview for the selected operation.
 *
 * Loads available post-processors on mount, then fetches a G-code preview
 * whenever the selected operation or post-processor changes.  An Export
 * button opens a save-file dialog and writes the G-code to disk.
 */

import { useEffect, useState } from 'react'
import { listPostProcessors, getGcodePreview, exportGcode } from '../../api/toolpath'
import { usePushNotification, useSelectedOperationId } from '../../store/projectStore'
import { toAppError } from '../../api/errors'
import { save } from '@tauri-apps/plugin-dialog'
import { Button } from '@/components/ui/button'
import { Download } from 'lucide-react'
import type { PostProcessorMeta } from '../../api/types'

const NO_TOOLPATH_MSG = 'No toolpath computed for this operation.'

export function GCodePreviewPanel() {
  const selectedOperationId = useSelectedOperationId()
  const pushNotification = usePushNotification()

  const [postProcessors, setPostProcessors] = useState<PostProcessorMeta[]>([])
  const [selectedPpId, setSelectedPpId] = useState<string | null>(null)
  const [gcode, setGcode] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  // Load post-processors on mount
  useEffect(() => {
    listPostProcessors()
      .then((pps) => {
        setPostProcessors(pps)
        setSelectedPpId(pps[0]?.id ?? null)
      })
      .catch((err: unknown) => {
        const e = toAppError(err)
        pushNotification(`Failed to load post-processors: ${e.message ?? e.kind}`)
      })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Fetch G-code preview when operation or post-processor selection changes
  useEffect(() => {
    if (!selectedOperationId || !selectedPpId) {
      setGcode(null)
      setLoading(false)
      return
    }
    setLoading(true)
    getGcodePreview(selectedOperationId, selectedPpId)
      .then((text) => {
        setGcode(text)
      })
      .catch((err: unknown) => {
        const e = toAppError(err)
        if (e.kind === 'NotFound') {
          setGcode(null)
        } else {
          pushNotification(`Failed to load G-code preview: ${e.message ?? e.kind}`)
        }
      })
      .finally(() => setLoading(false))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedOperationId, selectedPpId])

  async function handleExport() {
    if (!selectedOperationId || !selectedPpId) return
    const path = await save({ filters: [{ name: 'NC Files', extensions: ['nc'] }] })
    if (!path) return
    try {
      await exportGcode({
        operationIds: [selectedOperationId],
        postProcessorId: selectedPpId,
        outputPath: path,
        includeComments: true,
      })
    } catch (err: unknown) {
      const e = toAppError(err)
      pushNotification(`Export failed: ${e.message ?? e.kind}`)
    }
  }

  if (!selectedOperationId) {
    return (
      <p className="text-xs text-muted-foreground">Select an operation to preview G-code.</p>
    )
  }

  return (
    <div className="space-y-2">
      <select
        value={selectedPpId ?? ''}
        onChange={(e) => setSelectedPpId(e.target.value || null)}
        aria-label="Post-processor"
        className="h-7 w-full rounded-sm border border-border bg-input px-2 text-xs text-foreground"
      >
        {postProcessors.map((pp) => (
          <option key={pp.id} value={pp.id}>
            {pp.name}
          </option>
        ))}
      </select>
      <pre className="max-h-48 overflow-auto rounded-md bg-muted p-2 font-mono text-xs text-muted-foreground">
        {loading ? 'Loading...' : (gcode ?? NO_TOOLPATH_MSG)}
      </pre>
      <Button variant="outline" size="sm" onClick={handleExport} disabled={!gcode}>
        <Download className="mr-1 h-3.5 w-3.5" />
        Export
      </Button>
    </div>
  )
}
