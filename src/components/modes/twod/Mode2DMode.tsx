/**
 * Mode2DMode — top-level component for 2D Profiling mode.
 *
 * Manages file import, curve display, project settings (stock / safe height),
 * and tool library in an editing substate. The viewing substate (G-code preview)
 * is wired in by bead-14.
 */

import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { Canvas2D } from './Canvas2D'
import { ToolEditorList } from '../../tools/ToolEditorList'
import {
  loadTwodFile,
  getTwodCurves,
  setSafeHeight,
  setArtworkOrigin,
} from '../../../api/twodMode'
import { setStock } from '../../../api/stock'
import { deleteOperation } from '../../../api/operations'
import { listTools, deleteTool } from '../../../api/tools'
import { toAppError } from '../../../api/errors'
import { useProjectStore, usePushNotification } from '../../../store/projectStore'
import type { CurveSummary } from '../../../api/twodMode'
import type { BoxStock, Tool } from '../../../api/types'

type SubState = 'editing' | 'viewing'

export function Mode2DMode() {
  const snapshot = useProjectStore((s) => s.snapshot)
  const pushNotification = usePushNotification()

  // ── Sub-state ─────────────────────────────────────────────────────────────
  // NOTE: 'viewing' substate (G-code preview) is populated by bead-14.
  const [subState] = useState<SubState>('editing')

  // ── Curve data ────────────────────────────────────────────────────────────
  const [curves, setCurves] = useState<CurveSummary[]>([])
  const [curvePointsMap, setCurvePointsMap] = useState<Map<string, number[][]>>(new Map())
  const [unitSystem, setUnitSystem] = useState<'mm' | 'inches' | null>(null)
  const [loadedFileName, setLoadedFileName] = useState<string | null>(null)

  // ── Tool library ──────────────────────────────────────────────────────────
  const [tools, setTools] = useState<Tool[]>([])

  // ── Pending file load — persists across confirm → unit-selection steps ────
  const [pendingPath, setPendingPath] = useState<string | null>(null)
  const [replaceConfirmOpen, setReplaceConfirmOpen] = useState(false)
  const [svgUnitModalOpen, setSvgUnitModalOpen] = useState(false)

  // ── Stock field local state ───────────────────────────────────────────────
  const [topZStr, setTopZStr] = useState('')
  const [thicknessStr, setThicknessStr] = useState('')
  const [xDimStr, setXDimStr] = useState('')
  const [yDimStr, setYDimStr] = useState('')

  // ── Safe height local state ───────────────────────────────────────────────
  const [safeHeightStr, setSafeHeightStr] = useState('')

  // ── Derived values from snapshot ──────────────────────────────────────────
  const stockFromSnapshot = snapshot?.stock
  const artworkOrigin: [number, number] = snapshot?.artworkOrigin ?? [0, 0]
  const stockDims = stockFromSnapshot
    ? { width: stockFromSnapshot.width, depth: stockFromSnapshot.depth }
    : null
  const assignedIds = new Set(
    (snapshot?.operations ?? [])
      .filter((op) => op.curveId != null)
      .map((op) => op.curveId as string),
  )

  // ── On mount: restore state if project was loaded from disk ──────────────
  useEffect(() => {
    async function init() {
      try {
        const result = await getTwodCurves()
        if (result) {
          setCurves(result.curves)
          setCurvePointsMap(new Map(Object.entries(result.curvePoints)))
          setUnitSystem(result.unitSystem)
          setLoadedFileName('(loaded)')
        }
      } catch (e) {
        const err = toAppError(e)
        pushNotification(err.message ?? err.kind ?? 'Failed to load 2D curves')
      }
      try {
        const ts = await listTools()
        setTools(ts)
      } catch (_e) {
        // Non-fatal — tool list starts empty
      }
    }
    void init()
  }, [pushNotification])

  // ── Sync stock fields from snapshot ───────────────────────────────────────
  useEffect(() => {
    if (stockFromSnapshot?.type === 'box') {
      const s = stockFromSnapshot
      setTopZStr(String(s.origin.z + s.height))
      setThicknessStr(String(s.height))
      setXDimStr(String(s.width))
      setYDimStr(String(s.depth))
    }
  }, [stockFromSnapshot])

  // ── Sync safe height from snapshot ────────────────────────────────────────
  useEffect(() => {
    if (snapshot?.safeHeight != null) {
      setSafeHeightStr(String(snapshot.safeHeight))
    }
  }, [snapshot?.safeHeight])

  // ── File import logic ─────────────────────────────────────────────────────

  async function doLoad(path: string, unitHint: 'mm' | 'inches' | null) {
    let result
    try {
      result = await loadTwodFile(path, unitHint)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to load 2D file')
      return
    }
    // loadTwodFile succeeded — delete existing operations then update UI.
    // Read fresh state here (not the render-closure snapshot) because the
    // async loadTwodFile call above may have allowed the snapshot to update.
    // Use allSettled so a failed individual delete doesn't block the UI update;
    // the backend has already accepted the new file.
    const ops = useProjectStore.getState().snapshot?.operations ?? []
    await Promise.allSettled(ops.map((op) => deleteOperation(op.id)))
    setCurves(result.curves)
    setCurvePointsMap(new Map(Object.entries(result.curvePoints)))
    setUnitSystem(result.unitSystem)
    setLoadedFileName(path.split(/[\\/]/).pop() ?? path)
  }

  async function handlePickFile() {
    let chosen: string
    try {
      const result = await open({
        filters: [{ name: '2D Files', extensions: ['svg', 'dxf'] }],
        multiple: false,
      })
      if (typeof result !== 'string') return
      chosen = result
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to open file dialog')
      return
    }

    if (curves.length > 0) {
      // File already loaded — ask for confirmation before proceeding.
      setPendingPath(chosen)
      setReplaceConfirmOpen(true)
    } else if (chosen.toLowerCase().endsWith('.svg')) {
      // Need unit hint before loading.
      setPendingPath(chosen)
      setSvgUnitModalOpen(true)
    } else {
      await doLoad(chosen, null)
    }
  }

  async function handleReplaceConfirm() {
    setReplaceConfirmOpen(false)
    if (!pendingPath) return
    if (pendingPath.toLowerCase().endsWith('.svg')) {
      setSvgUnitModalOpen(true)
    } else {
      const path = pendingPath
      setPendingPath(null)
      await doLoad(path, null)
    }
  }

  function handleReplaceCancel() {
    setReplaceConfirmOpen(false)
    setPendingPath(null)
  }

  async function handleSvgUnitSelect(unit: 'mm' | 'inches') {
    setSvgUnitModalOpen(false)
    if (!pendingPath) return
    const path = pendingPath
    setPendingPath(null)
    await doLoad(path, unit)
  }

  function handleSvgUnitCancel() {
    setSvgUnitModalOpen(false)
    setPendingPath(null)
  }

  // ── Canvas handlers ───────────────────────────────────────────────────────

  function handleCurveSelect(_id: string | null) {
    // bead-13 adds operation creation logic here.
  }

  async function handleOriginChange(x: number, y: number) {
    try {
      await setArtworkOrigin(x, y)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to set artwork origin')
    }
  }

  // ── Stock handlers ────────────────────────────────────────────────────────

  async function handleStockChange(
    field: 'topZ' | 'thickness' | 'xDim' | 'yDim',
    value: string,
  ) {
    if (field === 'topZ') setTopZStr(value)
    else if (field === 'thickness') setThicknessStr(value)
    else if (field === 'xDim') setXDimStr(value)
    else setYDimStr(value)

    const topZ = parseFloat(field === 'topZ' ? value : topZStr)
    const thickness = parseFloat(field === 'thickness' ? value : thicknessStr)
    const width = parseFloat(field === 'xDim' ? value : xDimStr)
    const depth = parseFloat(field === 'yDim' ? value : yDimStr)

    if (isNaN(topZ) || isNaN(thickness) || isNaN(width) || isNaN(depth)) return

    const existing = stockFromSnapshot?.type === 'box' ? stockFromSnapshot : null
    const newStock: BoxStock = {
      type: 'box',
      origin: {
        x: existing?.origin.x ?? 0,
        y: existing?.origin.y ?? 0,
        z: topZ - thickness,
      },
      width,
      depth,
      height: thickness,
    }
    try {
      await setStock(newStock)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to set stock')
    }
  }

  async function handleSafeHeightChange(value: string) {
    setSafeHeightStr(value)
    const parsed = parseFloat(value)
    if (isNaN(parsed)) return
    try {
      await setSafeHeight(parsed)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to set safe height')
    }
  }

  // ── Tool handlers ─────────────────────────────────────────────────────────

  function handleToolEdit(_id: string) {
    // Tool editing is delegated to the ToolEditorWindow (separate window).
  }

  async function handleToolDelete(id: string) {
    try {
      await deleteTool(id)
      setTools((prev) => prev.filter((t) => t.id !== id))
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to delete tool')
    }
  }

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-screen bg-background text-foreground">
      {/* Left: Canvas area */}
      <div className="min-w-0 flex-1">
        {subState === 'editing' && curves.length > 0 && (
          <Canvas2D
            curves={curves}
            fullCurvePoints={curvePointsMap}
            artworkOffset={artworkOrigin}
            stockDims={stockDims}
            assignedCurveIds={assignedIds}
            onCurveSelect={handleCurveSelect}
            onArtworkOriginChange={handleOriginChange}
          />
        )}
      </div>

      {/* Right: Sidebar (~300px) */}
      <aside className="w-[300px] shrink-0 border-l border-border">
        <ScrollArea className="h-full">

          {/* Back button */}
          <div className="border-b border-border px-3 py-2">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => useProjectStore.getState().returnToSelector()}
            >
              ← Back
            </Button>
          </div>

          {subState === 'editing' && (
            <>
              {/* File import panel */}
              <SidebarSection title="File">
                <div className="flex flex-col gap-2">
                  <Button size="sm" className="w-full" onClick={() => void handlePickFile()}>
                    Load 2D File
                  </Button>
                  {loadedFileName && (
                    <p
                      className="truncate text-xs text-muted-foreground"
                      title={loadedFileName}
                    >
                      {loadedFileName} — {curves.length} curve{curves.length !== 1 ? 's' : ''}
                      {unitSystem && ` (${unitSystem})`}
                    </p>
                  )}
                </div>
              </SidebarSection>

              {/* Tool library panel */}
              <SidebarSection title="Tools">
                <ToolEditorList
                  tools={tools}
                  onEdit={handleToolEdit}
                  onDelete={(id) => void handleToolDelete(id)}
                />
              </SidebarSection>

              {/* Project settings: Stock + Safe Height */}
              <SidebarSection title="Project Settings">
                <div className="flex flex-col gap-3">
                  <div className="flex flex-col gap-1.5">
                    <span className="text-xs font-medium text-foreground">Stock</span>
                    <div className="flex flex-col gap-1">
                      <div className="flex flex-col gap-0.5">
                        <label className="text-xs text-muted-foreground">Top of stock Z</label>
                        <input
                          type="number"
                          aria-label="Top of stock Z"
                          value={topZStr}
                          onChange={(e) => void handleStockChange('topZ', e.target.value)}
                          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                          step="0.1"
                        />
                      </div>
                      <div className="flex flex-col gap-0.5">
                        <label className="text-xs text-muted-foreground">Thickness</label>
                        <input
                          type="number"
                          aria-label="Thickness"
                          value={thicknessStr}
                          onChange={(e) => void handleStockChange('thickness', e.target.value)}
                          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                          step="0.1"
                          min="0"
                        />
                      </div>
                      <div className="flex flex-col gap-0.5">
                        <label className="text-xs text-muted-foreground">X dimension</label>
                        <input
                          type="number"
                          aria-label="X dimension"
                          value={xDimStr}
                          onChange={(e) => void handleStockChange('xDim', e.target.value)}
                          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                          step="0.1"
                          min="0"
                        />
                      </div>
                      <div className="flex flex-col gap-0.5">
                        <label className="text-xs text-muted-foreground">Y dimension</label>
                        <input
                          type="number"
                          aria-label="Y dimension"
                          value={yDimStr}
                          onChange={(e) => void handleStockChange('yDim', e.target.value)}
                          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                          step="0.1"
                          min="0"
                        />
                      </div>
                    </div>
                  </div>
                  <div className="flex flex-col gap-0.5">
                    <label className="text-xs text-muted-foreground">Safe height (mm)</label>
                    <input
                      type="number"
                      aria-label="Safe height"
                      value={safeHeightStr}
                      onChange={(e) => void handleSafeHeightChange(e.target.value)}
                      className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
                      step="0.1"
                    />
                  </div>
                </div>
              </SidebarSection>
            </>
          )}

        </ScrollArea>
      </aside>

      {/* SVG unit selection modal */}
      {svgUnitModalOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          role="dialog"
          aria-modal="true"
          aria-labelledby="svg-unit-modal-title"
        >
          <div className="rounded-lg border border-border bg-background p-6 shadow-lg">
            <h2 id="svg-unit-modal-title" className="mb-2 text-sm font-semibold">
              Select unit system
            </h2>
            <p className="mb-4 text-xs text-muted-foreground">
              SVG files do not embed unit information. Choose the unit system used in this file.
            </p>
            <div className="flex gap-2">
              <Button size="sm" onClick={() => void handleSvgUnitSelect('mm')}>
                Millimeters (mm)
              </Button>
              <Button size="sm" variant="secondary" onClick={() => void handleSvgUnitSelect('inches')}>
                Inches
              </Button>
              <Button size="sm" variant="ghost" onClick={handleSvgUnitCancel}>
                Cancel
              </Button>
            </div>
          </div>
        </div>
      )}

      {/* Replace confirmation modal */}
      {replaceConfirmOpen && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/50"
          role="dialog"
          aria-modal="true"
          aria-labelledby="replace-confirm-title"
        >
          <div className="rounded-lg border border-border bg-background p-6 shadow-lg">
            <h2 id="replace-confirm-title" className="mb-2 text-sm font-semibold">
              Replace file?
            </h2>
            <p className="mb-4 text-xs text-muted-foreground">
              Loading a new file will clear all existing operations. Continue?
            </p>
            <div className="flex gap-2">
              <Button size="sm" onClick={() => void handleReplaceConfirm()}>
                Continue
              </Button>
              <Button size="sm" variant="ghost" onClick={handleReplaceCancel}>
                Cancel
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
