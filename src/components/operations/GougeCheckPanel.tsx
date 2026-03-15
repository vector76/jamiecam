/**
 * GougeCheckPanel — transient gouge-detection UI for surface finishing operations.
 *
 * Renders a "Check Gouges" button, displays pass/fail results, and offers
 * an "Auto-Lift" action when violations are found.  Results are held in
 * component state (not the project store) and cleared whenever the
 * toolpathVersion prop changes (indicating a toolpath recalculation).
 */

import { useEffect, useState } from 'react'
import { checkGouge, autoLift } from '../../api/toolpath'
import { toAppError } from '../../api/errors'
import { useProjectStore } from '../../store/projectStore'
import type { GougeCheckResult } from '../../api/types'

interface Props {
  operationId: string
  toolpathVersion?: number | string | null
}

export default function GougeCheckPanel({ operationId, toolpathVersion }: Props) {
  const [result, setResult] = useState<GougeCheckResult | null>(null)
  const [loading, setLoading] = useState(false)
  const pushNotification = useProjectStore((s) => s.pushNotification)

  // Clear stale results when toolpath is recalculated.
  useEffect(() => {
    setResult(null)
  }, [toolpathVersion])

  async function handleCheckGouge() {
    setLoading(true)
    try {
      const r = await checkGouge(operationId)
      setResult(r)
    } catch (e: unknown) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Gouge check failed')
    } finally {
      setLoading(false)
    }
  }

  async function handleAutoLift() {
    setLoading(true)
    try {
      const corrected = await autoLift(operationId)
      pushNotification(`Auto-lift corrected ${corrected} point${corrected === 1 ? '' : 's'}`)
      // Re-run gouge check to show updated status.
      const r = await checkGouge(operationId)
      setResult(r)
    } catch (e: unknown) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Auto-lift failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div style={{ marginTop: '0.5rem', borderTop: '1px solid #eee', paddingTop: '0.5rem' }}>
      <button onClick={() => void handleCheckGouge()} disabled={loading}>
        {loading ? 'Checking…' : 'Check Gouges'}
      </button>

      {result !== null && (
        <div style={{ marginTop: '0.25rem' }}>
          {result.passed ? (
            <span style={{ color: 'green' }}>&#x2714; No gouges</span>
          ) : (
            <div>
              <span style={{ color: 'red', fontWeight: 'bold' }}>
                {result.violations.length} violation{result.violations.length === 1 ? '' : 's'}
              </span>

              <details style={{ marginTop: '0.25rem' }}>
                <summary style={{ cursor: 'pointer' }}>Show violations</summary>
                <ul style={{ fontSize: '0.85em', margin: '0.25rem 0', paddingLeft: '1.25rem' }}>
                  {result.violations.map((v, i) => (
                    <li key={i}>
                      ({v.position[0].toFixed(3)}, {v.position[1].toFixed(3)}, {v.position[2].toFixed(3)})
                      &mdash; depth {v.gougeDepth.toFixed(4)} mm
                    </li>
                  ))}
                </ul>
              </details>

              <button
                onClick={() => void handleAutoLift()}
                disabled={loading}
                style={{ marginTop: '0.25rem' }}
              >
                Auto-Lift
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
