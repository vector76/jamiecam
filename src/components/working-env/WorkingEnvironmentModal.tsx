/**
 * WorkingEnvironmentModal — minimal CRUD UI over the persisted working
 * environment.
 *
 * Surfaces three editable collections (setups, tools, availability matrix)
 * plus a one-of selector for the "active" setup. Everything is persisted
 * through the workingEnv module so the user's choices survive reloads.
 *
 * Field coverage is intentionally minimal — only the name is exposed in
 * the create/rename flow; other fields (workspace, kinematics, feeds &
 * speeds…) take their defaults until a dedicated property editor lands.
 */

import { useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  loadActiveSetupId,
  loadWorkingEnv,
  saveActiveSetupId,
  saveWorkingEnv,
} from '../../persistence/workingEnv'
import type { AvailabilityMatrix, MachineSetup, Tool, WorkingEnvironment } from '../../api/types'

interface WorkingEnvironmentModalProps {
  open: boolean
  onClose: () => void
  /** Test seam — defaults to crypto.randomUUID(). */
  newId?: () => string
}

const EMPTY_ENV: WorkingEnvironment = { setups: [], tools: [], availability: [] }

function defaultNewId(): string {
  return crypto.randomUUID()
}

function freshSetup(id: string): MachineSetup {
  return {
    id,
    name: 'New Setup',
    workspace: { origin: { x: 0, y: 0, z: 0 }, width: 300, depth: 200, height: 80 },
    kinematics: '3-axis-router',
    postProcessor: 'grbl-1.1',
    safety: { safeZ: 5, rapidFeedRate: 3000 },
  }
}

function freshTool(id: string): Tool {
  return {
    id,
    name: 'New Tool',
    diameter: 3.175,
    fluteCount: 2,
    length: 38,
    material: 'carbide',
    recommended: { spindleRpm: 18000, feedRate: 800, plungeRate: 200 },
  }
}

function pairsEqual(a: { setupId: string; toolId: string }, setupId: string, toolId: string) {
  return a.setupId === setupId && a.toolId === toolId
}

export function WorkingEnvironmentModal({
  open,
  onClose,
  newId = defaultNewId,
}: WorkingEnvironmentModalProps) {
  const [env, setEnv] = useState<WorkingEnvironment>(EMPTY_ENV)
  const [activeSetupId, setActiveSetupIdState] = useState<string | null>(null)
  const [loaded, setLoaded] = useState(false)

  useEffect(() => {
    if (!open) return
    // Reset loaded each time the modal opens so a re-open re-reads from DB
    // (and so the body stays hidden behind the action buttons until the
    // async load resolves — which is what prevents user clicks from racing
    // against the initial load and getting overwritten).
    setLoaded(false)
    let cancelled = false
    void (async () => {
      const [loadedEnv, loadedActive] = await Promise.all([loadWorkingEnv(), loadActiveSetupId()])
      if (cancelled) return
      setEnv(loadedEnv)
      setActiveSetupIdState(loadedActive)
      setLoaded(true)
    })()
    return () => {
      cancelled = true
    }
  }, [open])

  if (!open) return null

  async function persist(next: WorkingEnvironment) {
    setEnv(next)
    await saveWorkingEnv(next)
  }

  async function handleAddSetup() {
    const setup = freshSetup(newId())
    await persist({ ...env, setups: [...env.setups, setup] })
  }

  async function handleRenameSetup(id: string, name: string) {
    const setups = env.setups.map((s) => (s.id === id ? { ...s, name } : s))
    await persist({ ...env, setups })
  }

  async function handleDeleteSetup(id: string) {
    const setups = env.setups.filter((s) => s.id !== id)
    const availability = env.availability.filter((p) => p.setupId !== id)
    await persist({ ...env, setups, availability })
    if (activeSetupId === id) {
      setActiveSetupIdState(null)
      await saveActiveSetupId(null)
    }
  }

  async function handleAddTool() {
    const tool = freshTool(newId())
    await persist({ ...env, tools: [...env.tools, tool] })
  }

  async function handleRenameTool(id: string, name: string) {
    const tools = env.tools.map((t) => (t.id === id ? { ...t, name } : t))
    await persist({ ...env, tools })
  }

  async function handleDeleteTool(id: string) {
    const tools = env.tools.filter((t) => t.id !== id)
    const availability = env.availability.filter((p) => p.toolId !== id)
    await persist({ ...env, tools, availability })
  }

  async function handleToggleAvailability(setupId: string, toolId: string) {
    const has = env.availability.some((p) => pairsEqual(p, setupId, toolId))
    const availability: AvailabilityMatrix = has
      ? env.availability.filter((p) => !pairsEqual(p, setupId, toolId))
      : [...env.availability, { setupId, toolId }]
    await persist({ ...env, availability })
  }

  async function handlePickActive(id: string) {
    setActiveSetupIdState(id)
    await saveActiveSetupId(id)
  }

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Working Environment"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
    >
      <div className="border-border bg-background text-foreground flex max-h-full w-full max-w-2xl flex-col overflow-hidden rounded-lg border shadow-lg">
        <header className="border-border flex items-center justify-between border-b px-4 py-2">
          <h2 className="text-sm font-semibold tracking-wide uppercase">Working Environment</h2>
          <Button size="sm" variant="ghost" onClick={onClose} aria-label="Close">
            ✕
          </Button>
        </header>

        <div className="flex-1 space-y-6 overflow-auto p-4">
          {!loaded ? (
            <p className="text-muted-foreground text-xs" role="status">
              Loading…
            </p>
          ) : (
            <>
              <section aria-labelledby="setups-heading">
                <div className="mb-2 flex items-center justify-between">
                  <h3 id="setups-heading" className="text-xs font-semibold uppercase">
                    Setups
                  </h3>
                  <Button size="xs" onClick={handleAddSetup}>
                    Add Setup
                  </Button>
                </div>
                {env.setups.length === 0 ? (
                  <p className="text-muted-foreground text-xs">No setups yet.</p>
                ) : (
                  <ul className="flex flex-col gap-1">
                    {env.setups.map((s) => (
                      <li key={s.id} className="flex items-center gap-2">
                        <input
                          type="radio"
                          name="active-setup"
                          checked={activeSetupId === s.id}
                          onChange={() => void handlePickActive(s.id)}
                          aria-label={`Set active: ${s.name}`}
                          title="Active setup"
                        />
                        <input
                          type="text"
                          value={s.name}
                          onChange={(e) =>
                            setEnv((prev) => ({
                              ...prev,
                              setups: prev.setups.map((x) =>
                                x.id === s.id ? { ...x, name: e.target.value } : x,
                              ),
                            }))
                          }
                          onBlur={(e) => void handleRenameSetup(s.id, e.target.value)}
                          aria-label={`Setup name (${s.id})`}
                          className="border-border bg-background flex-1 rounded border px-2 py-1 text-xs"
                        />
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => void handleDeleteSetup(s.id)}
                          aria-label={`Delete setup ${s.name}`}
                        >
                          Delete
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section aria-labelledby="tools-heading">
                <div className="mb-2 flex items-center justify-between">
                  <h3 id="tools-heading" className="text-xs font-semibold uppercase">
                    Tools
                  </h3>
                  <Button size="xs" onClick={handleAddTool}>
                    Add Tool
                  </Button>
                </div>
                {env.tools.length === 0 ? (
                  <p className="text-muted-foreground text-xs">No tools yet.</p>
                ) : (
                  <ul className="flex flex-col gap-1">
                    {env.tools.map((t) => (
                      <li key={t.id} className="flex items-center gap-2">
                        <input
                          type="text"
                          value={t.name}
                          onChange={(e) =>
                            setEnv((prev) => ({
                              ...prev,
                              tools: prev.tools.map((x) =>
                                x.id === t.id ? { ...x, name: e.target.value } : x,
                              ),
                            }))
                          }
                          onBlur={(e) => void handleRenameTool(t.id, e.target.value)}
                          aria-label={`Tool name (${t.id})`}
                          className="border-border bg-background flex-1 rounded border px-2 py-1 text-xs"
                        />
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => void handleDeleteTool(t.id)}
                          aria-label={`Delete tool ${t.name}`}
                        >
                          Delete
                        </Button>
                      </li>
                    ))}
                  </ul>
                )}
              </section>

              <section aria-labelledby="availability-heading">
                <h3 id="availability-heading" className="mb-2 text-xs font-semibold uppercase">
                  Availability
                </h3>
                {env.setups.length === 0 || env.tools.length === 0 ? (
                  <p className="text-muted-foreground text-xs">
                    Add at least one setup and one tool to edit the availability matrix.
                  </p>
                ) : (
                  <table className="w-full border-collapse text-xs">
                    <thead>
                      <tr>
                        <th className="border-border border px-2 py-1 text-left"></th>
                        {env.tools.map((t) => (
                          <th
                            key={t.id}
                            className="border-border border px-2 py-1 text-left font-normal"
                          >
                            {t.name}
                          </th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {env.setups.map((s) => (
                        <tr key={s.id}>
                          <th
                            scope="row"
                            className="border-border border px-2 py-1 text-left font-normal"
                          >
                            {s.name}
                          </th>
                          {env.tools.map((t) => {
                            const checked = env.availability.some((p) => pairsEqual(p, s.id, t.id))
                            return (
                              <td key={t.id} className="border-border border px-2 py-1 text-center">
                                <input
                                  type="checkbox"
                                  checked={checked}
                                  onChange={() => void handleToggleAvailability(s.id, t.id)}
                                  aria-label={`${s.name} / ${t.name}`}
                                />
                              </td>
                            )
                          })}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </section>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
