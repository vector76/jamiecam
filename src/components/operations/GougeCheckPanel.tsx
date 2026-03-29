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
import { Button } from '@/components/ui/button'
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
    <div className="mt-2 border-t border-border pt-2">
      <Button
        variant="outline"
        size="sm"
        onClick={() => void handleCheckGouge()}
        disabled={loading}
      >
        {loading ? 'Checking...' : 'Check Gouges'}
      </Button>

      {result !== null && (
        <div className="mt-1">
          {result.passed ? (
            <span className="text-sm text-success">&#x2714; No gouges</span>
          ) : (
            <div className="space-y-1">
              <span className="text-sm font-bold text-destructive">
                {result.violations.length} violation{result.violations.length === 1 ? '' : 's'}
              </span>

              <details className="text-xs">
                <summary className="cursor-pointer text-muted-foreground hover:text-foreground">
                  Show violations
                </summary>
                <ul className="mt-1 space-y-0.5 pl-4 text-muted-foreground">
                  {result.violations.map((v, i) => (
                    <li key={i}>
                      ({v.position[0].toFixed(3)}, {v.position[1].toFixed(3)},{' '}
                      {v.position[2].toFixed(3)}) &mdash; depth {v.gougeDepth.toFixed(4)} mm
                    </li>
                  ))}
                </ul>
              </details>

              <Button
                variant="outline"
                size="sm"
                onClick={() => void handleAutoLift()}
                disabled={loading}
              >
                Auto-Lift
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
