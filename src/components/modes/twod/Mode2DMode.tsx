/**
 * Mode2DMode — top-level component for 2D Profiling mode.
 *
 * Manages file import, curve display, project settings (stock / safe height),
 * tool library, per-curve operations panel, and G-code generation.
 */

import { useState, useEffect } from 'react'
import { open } from '@tauri-apps/plugin-dialog'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Button } from '@/components/ui/button'
import { Canvas2D } from './Canvas2D'
import { Viewport } from '../../../viewport/Viewport'
import { ToolEditorList } from '../../tools/ToolEditorList'
import {
  loadTwodFile,
  getTwodCurves,
  setSafeHeight,
  setArtworkOrigin,
  generate2dGcode,
} from '../../../api/twodMode'
import { setStock } from '../../../api/stock'
import { addOperation, editOperation, deleteOperation, listOperations } from '../../../api/operations'
import { listTools, deleteTool } from '../../../api/tools'
import { toAppError } from '../../../api/errors'
import { useProjectStore, usePushNotification } from '../../../store/projectStore'
import { useViewportStore } from '../../../store/viewportStore'
import type { CurveSummary, Generate2dResult } from '../../../api/twodMode'
import type { BoxStock, Tool, Profile2dParams, Operation } from '../../../api/types'

type SubState = 'editing' | 'viewing'

// ── OperationEditForm ─────────────────────────────────────────────────────────

interface OperationEditFormProps {
  opId: string
  tools: Tool[]
  stockTopZ: number | null
  onEdit: (op: Operation, patch: Partial<Profile2dParams & { toolId: string }>) => Promise<void>
  onRemove: () => void
}

function OperationEditForm({
  opId,
  tools,
  stockTopZ,
  onEdit,
  onRemove,
}: OperationEditFormProps) {
  const pushNotification = usePushNotification()
  const [allOps, setAllOps] = useState<Operation[]>([])

  async function refreshOps() {
    try {
      const ops = await listOperations()
      setAllOps(ops)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to load operation')
    }
  }

  useEffect(() => {
    let cancelled = false
    async function load() {
      try {
        const ops = await listOperations()
        if (!cancelled) setAllOps(ops)
      } catch (e) {
        const err = toAppError(e)
        pushNotification(err.message ?? err.kind ?? 'Failed to load operation')
      }
    }
    void load()
    return () => { cancelled = true }
  }, [opId, pushNotification])

  const op = allOps.find((o) => o.id === opId) ?? null

  // Multi-tool: advisory when Profile2d ops use more than one distinct tool ID.
  const profile2dToolIds = new Set(
    allOps.filter((o) => o.type === 'profile_2d').map((o) => o.toolId),
  )
  const multiTool = profile2dToolIds.size > 1

  if (op == null) return null

  const params = op.params as Profile2dParams
  const bottomOfCut = params.topOfCut - params.depthOfCut
  const topOfCutBelowStock = stockTopZ != null && params.topOfCut <= stockTopZ

  async function commitPatch(patch: Partial<Profile2dParams & { toolId: string }>) {
    if (op == null) return
    await onEdit(op, patch)
    await refreshOps()
  }

  function numericField(
    label: string,
    ariaLabel: string,
    value: number,
    patchKey: keyof Profile2dParams,
    opts?: { min?: number; step?: number },
  ) {
    return (
      <div className="flex flex-col gap-0.5">
        <label className="text-xs text-muted-foreground">{label}</label>
        <input
          type="number"
          aria-label={ariaLabel}
          defaultValue={value}
          key={`${opId}-${patchKey}-${value}`}
          onBlur={(e) => {
            const parsed = parseFloat(e.target.value)
            if (!isNaN(parsed)) void commitPatch({ [patchKey]: parsed })
          }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              const parsed = parseFloat((e.target as HTMLInputElement).value)
              if (!isNaN(parsed)) void commitPatch({ [patchKey]: parsed })
            }
          }}
          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
          step={opts?.step ?? 0.1}
          min={opts?.min}
        />
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-2">
      {/* Cut type */}
      <div className="flex flex-col gap-0.5">
        <span className="text-xs text-muted-foreground">Cut type</span>
        <div className="flex gap-3" role="radiogroup" aria-label="Cut type">
          {(['inside', 'outside', 'on_line'] as const).map((ct) => (
            <label key={ct} className="flex items-center gap-1 text-xs cursor-pointer">
              <input
                type="radio"
                name={`cut-type-${opId}`}
                value={ct}
                checked={params.cutType === ct}
                onChange={() => void commitPatch({ cutType: ct })}
              />
              {ct === 'inside' ? 'Inside' : ct === 'outside' ? 'Outside' : 'On-line'}
            </label>
          ))}
        </div>
      </div>

      {/* Direction */}
      <div className="flex flex-col gap-0.5">
        <span className="text-xs text-muted-foreground">Direction</span>
        <div className="flex gap-3" role="radiogroup" aria-label="Direction">
          {(['climb', 'conventional'] as const).map((dir) => (
            <label key={dir} className="flex items-center gap-1 text-xs cursor-pointer">
              <input
                type="radio"
                name={`direction-${opId}`}
                value={dir}
                checked={params.direction === dir}
                onChange={() => void commitPatch({ direction: dir })}
              />
              {dir === 'climb' ? 'Climb' : 'Conventional'}
            </label>
          ))}
        </div>
      </div>

      {/* Tool dropdown */}
      <div className="flex flex-col gap-0.5">
        <label htmlFor={`tool-select-${opId}`} className="text-xs text-muted-foreground">Tool</label>
        <select
          key={`${opId}-toolId-${op.toolId}`}
          id={`tool-select-${opId}`}
          aria-label="Tool"
          defaultValue={op.toolId}
          onChange={(e) => void commitPatch({ toolId: e.target.value })}
          className="h-7 w-full rounded-sm border border-border bg-background px-1 text-xs"
        >
          {tools.map((t) => (
            <option key={t.id} value={t.id}>
              {t.name} (⌀{t.diameter})
            </option>
          ))}
        </select>
      </div>

      {/* Numeric fields */}
      {numericField('Top of cut (Z)', 'Top of cut', params.topOfCut, 'topOfCut')}

      {topOfCutBelowStock && (
        <div
          role="status"
          className="rounded border border-amber-400 bg-amber-50 px-2 py-1 text-xs text-amber-700"
        >
          Top of cut is at or below stock top — check Z values.
        </div>
      )}

      {numericField('Depth of cut', 'Depth of cut', params.depthOfCut, 'depthOfCut', { min: 0 })}
      {numericField('Step-down', 'Step-down', params.stepDown, 'stepDown', { min: 0 })}
      {numericField('Feed rate', 'Feed rate', params.feedRate, 'feedRate', { min: 0, step: 1 })}

      {/* Bottom of cut (read-only) */}
      <div className="flex flex-col gap-0.5">
        <span className="text-xs text-muted-foreground">Bottom of cut</span>
        <span className="text-xs" aria-label="Bottom of cut">{bottomOfCut.toFixed(3)}</span>
      </div>

      {multiTool && (
        <div
          role="status"
          className="rounded border border-amber-400 bg-amber-50 px-2 py-1 text-xs text-amber-700"
        >
          Multiple tools assigned — generation will fail until one tool is used across all operations.
        </div>
      )}

      <Button
        size="sm"
        variant="destructive"
        className="w-full mt-1"
        onClick={onRemove}
      >
        Remove operation
      </Button>
    </div>
  )
}

// ── Mode2DMode ────────────────────────────────────────────────────────────────

export function Mode2DMode() {
  const snapshot = useProjectStore((s) => s.snapshot)
  const pushNotification = usePushNotification()

  // ── Sub-state ─────────────────────────────────────────────────────────────
  const [subState, setSubState] = useState<SubState>('editing')

  // ── G-code result ─────────────────────────────────────────────────────────
  const [generate2dResult, setGenerate2dResult] = useState<Generate2dResult | null>(null)

  // ── Curve data ────────────────────────────────────────────────────────────
  const [curves, setCurves] = useState<CurveSummary[]>([])
  const [curvePointsMap, setCurvePointsMap] = useState<Map<string, number[][]>>(new Map())
  const [unitSystem, setUnitSystem] = useState<'mm' | 'inches' | null>(null)
  const [loadedFileName, setLoadedFileName] = useState<string | null>(null)

  // ── Tool library ──────────────────────────────────────────────────────────
  const [tools, setTools] = useState<Tool[]>([])

  // ── Selected curve ────────────────────────────────────────────────────────
  const [selectedCurveId, setSelectedCurveId] = useState<string | null>(null)

  // ── Generate state ────────────────────────────────────────────────────────
  const [generating, setGenerating] = useState(false)
  const [generateError, setGenerateError] = useState<string | null>(null)

  // ── Pending file load ─────────────────────────────────────────────────────
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

  const selectedOpSummary =
    selectedCurveId != null
      ? (snapshot?.operations ?? []).find(
          (op) => op.operationType === 'profile_2d' && op.curveId === selectedCurveId,
        ) ?? null
      : null

  const hasEnabledProfile2dOp = (snapshot?.operations ?? []).some(
    (op) => op.operationType === 'profile_2d' && op.enabled,
  )

  const stockTopZ =
    stockFromSnapshot?.type === 'box'
      ? stockFromSnapshot.origin.z + stockFromSnapshot.height
      : null

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
      } catch {
        // Non-fatal
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

  // ── Viewport lifecycle ─────────────────────────────────────────────────────
  useEffect(() => {
    useViewportStore.getState().setToolpathGeometry(null)
    return () => {
      useViewportStore.getState().setToolpathGeometry(null)
    }
  }, [])

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
    const ops = useProjectStore.getState().snapshot?.operations ?? []
    await Promise.allSettled(ops.map((op) => deleteOperation(op.id)))
    setSelectedCurveId(null)
    setGenerateError(null)
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
      setPendingPath(chosen)
      setReplaceConfirmOpen(true)
    } else if (chosen.toLowerCase().endsWith('.svg')) {
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

  function handleCurveSelect(id: string | null) {
    setSelectedCurveId(id)
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

  // ── Operation handlers ────────────────────────────────────────────────────

  async function handleAddOperation() {
    if (!selectedCurveId || tools.length === 0) return
    const params: Profile2dParams = {
      curveId: selectedCurveId,
      cutType: 'outside',
      direction: 'climb',
      topOfCut: snapshot?.safeHeight ?? 5.0,
      depthOfCut: 3.0,
      stepDown: 1.0,
      feedRate: 1000.0,
    }
    try {
      await addOperation({
        name: 'Profile 2D',
        enabled: true,
        toolId: tools[0].id,
        type: 'profile_2d',
        params,
      })
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to add operation')
    }
  }

  async function handleEditOperation(op: Operation, patch: Partial<Profile2dParams & { toolId: string }>) {
    const currentParams = op.params as Profile2dParams
    const newParams: Profile2dParams = {
      curveId: currentParams.curveId,
      cutType: patch.cutType ?? currentParams.cutType,
      direction: patch.direction ?? currentParams.direction,
      topOfCut: patch.topOfCut ?? currentParams.topOfCut,
      depthOfCut: patch.depthOfCut ?? currentParams.depthOfCut,
      stepDown: patch.stepDown ?? currentParams.stepDown,
      feedRate: patch.feedRate ?? currentParams.feedRate,
    }
    try {
      await editOperation(op.id, {
        name: op.name,
        enabled: op.enabled,
        toolId: patch.toolId ?? op.toolId,
        type: 'profile_2d',
        params: newParams,
      })
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to update operation')
    }
  }

  async function handleRemoveOperation(opId: string) {
    try {
      await deleteOperation(opId)
    } catch (e) {
      const err = toAppError(e)
      pushNotification(err.message ?? err.kind ?? 'Failed to remove operation')
    }
  }

  // ── Generate G-code ───────────────────────────────────────────────────────

  async function handleGenerate() {
    setGenerating(true)
    setGenerateError(null)
    try {
      const result = await generate2dGcode('grbl')
      setGenerate2dResult(result)
      setSubState('viewing')
      useViewportStore.getState().setToolpathGeometry(result.lineGeometry)
    } catch (e) {
      const err = toAppError(e)
      setGenerateError(err.message ?? err.kind ?? 'Generation failed')
    } finally {
      setGenerating(false)
    }
  }

  // ── Back to editing ───────────────────────────────────────────────────────

  function handleBackToEditing() {
    setSubState('editing')
    setGenerate2dResult(null)
    useViewportStore.getState().setToolpathGeometry(null)
  }

  // ── Operations panel renderer ─────────────────────────────────────────────

  function renderOperationsPanel() {
    if (selectedCurveId == null) {
      return (
        <p className="text-xs text-muted-foreground">
          Click a closed curve on the canvas to assign a cut operation.
        </p>
      )
    }

    if (selectedOpSummary == null) {
      return (
        <div className="flex flex-col gap-2">
          <Button
            size="sm"
            className="w-full"
            disabled={tools.length === 0}
            onClick={() => void handleAddOperation()}
          >
            Add operation
          </Button>
          {tools.length === 0 && (
            <p className="text-xs text-muted-foreground">Add a tool to the project first.</p>
          )}
        </div>
      )
    }

    return (
      <OperationEditForm
        opId={selectedOpSummary.id}
        tools={tools}
        stockTopZ={stockTopZ}
        onEdit={handleEditOperation}
        onRemove={() => void handleRemoveOperation(selectedOpSummary.id)}
      />
    )
  }

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className="flex h-screen bg-background text-foreground">
      {/* Left: Canvas area */}
      <div className="min-w-0 flex-1 flex flex-col overflow-hidden">
        {subState === 'viewing' && generate2dResult && generate2dResult.warnings.length > 0 && (
          <div
            role="status"
            className="shrink-0 rounded border border-amber-400 bg-amber-50 px-2 py-1.5 text-xs text-amber-700"
          >
            {`Warnings: ${generate2dResult.warnings.join(', ')}`}
          </div>
        )}
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
        {subState === 'viewing' && generate2dResult && (
          <Viewport className="flex-1" />
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

          {subState === 'viewing' && generate2dResult && (
            <div className="flex flex-col gap-3">
              {/* Back to 2D Canvas */}
              <div className="border-b border-border px-3 py-2">
                <Button
                  size="sm"
                  className="w-full"
                  onClick={handleBackToEditing}
                >
                  ← Back to 2D Canvas
                </Button>
              </div>

              {/* G-code preview */}
              <div className="px-3">
                <span className="text-xs font-medium text-foreground">G-code</span>
                <pre className="mt-1 max-h-64 overflow-auto rounded-md bg-muted p-2 font-mono text-xs text-muted-foreground">
                  {generate2dResult.gcode}
                </pre>
              </div>

              {/* Stats */}
              <div className="px-3 text-xs text-muted-foreground">
                <p>Points: {generate2dResult.stats.totalPointCount}</p>
                <p>Passes: {generate2dResult.stats.totalPassCount}</p>
                <p>Path: {generate2dResult.stats.totalPathLengthMm.toFixed(1)} mm</p>
              </div>
            </div>
          )}

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

              {/* Operations panel */}
              <SidebarSection title="Operation">
                {renderOperationsPanel()}
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

              {/* Generate G-code button */}
              <div className="px-3 py-3 border-t border-border">
                <Button
                  size="sm"
                  className="w-full"
                  disabled={generating || curves.length === 0 || !hasEnabledProfile2dOp}
                  onClick={() => void handleGenerate()}
                >
                  {generating ? 'Generating…' : 'Generate G-code'}
                </Button>
                {generateError && (
                  <div
                    role="alert"
                    className="mt-2 rounded border border-red-400 bg-red-50 px-2 py-1.5 text-xs text-red-700"
                  >
                    {generateError}
                  </div>
                )}
              </div>
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
