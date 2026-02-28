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
    return <p>Select an operation to preview G-code.</p>
  }

  return (
    <div>
      <select
        value={selectedPpId ?? ''}
        onChange={(e) => setSelectedPpId(e.target.value || null)}
        aria-label="Post-processor"
      >
        {postProcessors.map((pp) => (
          <option key={pp.id} value={pp.id}>{pp.name}</option>
        ))}
      </select>
      <pre>{loading ? 'Loading…' : (gcode ?? NO_TOOLPATH_MSG)}</pre>
      <button onClick={handleExport} disabled={!gcode}>
        Export…
      </button>
    </div>
  )
}
