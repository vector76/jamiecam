import { useRef, useEffect, useCallback, useState } from 'react'
import { useCanvas2dStore } from '../../../store/canvas2dStore'
import type { CurveSummary } from '../../../api/twodMode'
import { worldToScreen, screenToWorld, autoFitTransform } from './coordTransform'

export interface Canvas2DProps {
  curves: CurveSummary[]
  fullCurvePoints: Map<string, number[][]> // curveId -> [[x,y],...] for rendering polylines
  artworkOffset: [number, number]
  stockDims: { width: number; depth: number } | null
  assignedCurveIds: Set<string> // from project snapshot operations with curveId
  onCurveSelect: (id: string | null) => void
  onArtworkOriginChange: (x: number, y: number) => void // called on drag release
}

/** Minimum distance from a point (px, py) to the line segment (ax,ay)-(bx,by) in screen space. */
function distToSegment(
  px: number,
  py: number,
  ax: number,
  ay: number,
  bx: number,
  by: number,
): number {
  const dx = bx - ax
  const dy = by - ay
  const lenSq = dx * dx + dy * dy
  if (lenSq === 0) return Math.sqrt((px - ax) ** 2 + (py - ay) ** 2)
  const t = Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / lenSq))
  return Math.sqrt((px - ax - t * dx) ** 2 + (py - ay - t * dy) ** 2)
}

export function Canvas2D({
  curves,
  fullCurvePoints,
  artworkOffset,
  stockDims,
  assignedCurveIds,
  onCurveSelect,
  onArtworkOriginChange,
}: Canvas2DProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const [size, setSize] = useState({ width: 0, height: 0 })
  const [originDrag, setOriginDrag] = useState<{ x: number; y: number } | null>(null)
  const hasAutoFitted = useRef(false)

  const { panOffset, zoom, selectedCurveId, setPanOffset, setZoom, setSelectedCurveId } =
    useCanvas2dStore()

  // Drag interaction state — kept in a ref to avoid triggering re-renders on every mousemove.
  const dragState = useRef({
    type: null as 'pan' | 'origin' | null,
    startX: 0,
    startY: 0,
    startPanX: 0,
    startPanY: 0,
    /** Euclidean distance from drag-start to current pointer position. */
    totalMovement: 0,
  })

  // ── Resize observer ──────────────────────────────────────────────────────────

  useEffect(() => {
    const container = containerRef.current
    if (!container) return
    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect
        setSize({ width, height })
      }
    })
    observer.observe(container)
    return () => observer.disconnect()
  }, [])

  // ── Auto-fit on first render with curves ────────────────────────────────────

  useEffect(() => {
    if (!hasAutoFitted.current && curves.length > 0 && size.width > 0 && size.height > 0) {
      const fit = autoFitTransform(curves, stockDims, size.width, size.height)
      setPanOffset(fit.panOffset)
      setZoom(fit.zoom)
      hasAutoFitted.current = true
    }
  }, [curves, stockDims, size, setPanOffset, setZoom])

  // ── Rendering ────────────────────────────────────────────────────────────────

  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const { width, height } = size
    canvas.width = width
    canvas.height = height

    ctx.clearRect(0, 0, width, height)

    // Stock rectangle
    if (stockDims) {
      const tl = worldToScreen(0, stockDims.depth, panOffset, zoom, height)
      const br = worldToScreen(stockDims.width, 0, panOffset, zoom, height)
      ctx.fillStyle = 'rgba(180, 200, 220, 0.15)'
      ctx.fillRect(tl.x, tl.y, br.x - tl.x, br.y - tl.y)
      ctx.strokeStyle = 'rgba(100, 140, 180, 0.7)'
      ctx.lineWidth = 1
      ctx.strokeRect(tl.x, tl.y, br.x - tl.x, br.y - tl.y)
    }

    // Work origin crosshair at world (0, 0)
    const origin = worldToScreen(0, 0, panOffset, zoom, height)
    ctx.strokeStyle = '#888'
    ctx.lineWidth = 1
    ctx.setLineDash([])
    ctx.beginPath()
    ctx.moveTo(origin.x - 15, origin.y)
    ctx.lineTo(origin.x + 15, origin.y)
    ctx.moveTo(origin.x, origin.y - 15)
    ctx.lineTo(origin.x, origin.y + 15)
    ctx.stroke()

    // Helper to draw a polyline from raw world-space points
    const drawPolyline = (rawPoints: number[][], isClosed: boolean) => {
      if (rawPoints.length === 0) return
      const pts = rawPoints.map((pt) =>
        worldToScreen(pt[0] + artworkOffset[0], pt[1] + artworkOffset[1], panOffset, zoom, height),
      )
      ctx.beginPath()
      ctx.moveTo(pts[0].x, pts[0].y)
      for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i].x, pts[i].y)
      if (isClosed && pts.length > 1) ctx.closePath()
      ctx.stroke()
    }

    // Draw all curves; defer the selected one so it renders on top
    let selectedEntry: { curve: CurveSummary; rawPoints: number[][] } | null = null

    for (const curve of curves) {
      const curveId = curve.id
      const rawPoints = fullCurvePoints.get(curveId)
      if (!rawPoints || rawPoints.length === 0) continue

      if (curveId === selectedCurveId) {
        selectedEntry = { curve, rawPoints }
      }

      if (!curve.isClosed) {
        ctx.strokeStyle = 'rgba(150, 150, 150, 0.7)'
        ctx.lineWidth = 1
        ctx.setLineDash([4, 4])
      } else if (assignedCurveIds.has(curveId)) {
        ctx.strokeStyle = '#44bb44'
        ctx.lineWidth = 1
        ctx.setLineDash([])
      } else {
        ctx.strokeStyle = '#6699cc'
        ctx.lineWidth = 1
        ctx.setLineDash([])
      }
      drawPolyline(rawPoints, curve.isClosed)
    }

    ctx.setLineDash([])

    // Selected curve drawn last with thick highlight stroke
    if (selectedEntry) {
      ctx.strokeStyle = '#ffaa00'
      ctx.lineWidth = 3
      ctx.setLineDash([])
      drawPolyline(selectedEntry.rawPoints, selectedEntry.curve.isClosed)
    }

    // Origin drag preview
    if (originDrag) {
      const previewPos = worldToScreen(originDrag.x, originDrag.y, panOffset, zoom, height)
      ctx.strokeStyle = '#ff6600'
      ctx.lineWidth = 2
      ctx.setLineDash([3, 3])
      ctx.beginPath()
      ctx.moveTo(previewPos.x - 15, previewPos.y)
      ctx.lineTo(previewPos.x + 15, previewPos.y)
      ctx.moveTo(previewPos.x, previewPos.y - 15)
      ctx.lineTo(previewPos.x, previewPos.y + 15)
      ctx.stroke()
      ctx.setLineDash([])
    }
  }, [
    curves,
    fullCurvePoints,
    artworkOffset,
    stockDims,
    assignedCurveIds,
    selectedCurveId,
    panOffset,
    zoom,
    size,
    originDrag,
  ])

  // ── Event handlers ───────────────────────────────────────────────────────────

  const handleMouseDown = useCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current
    if (!canvas) return
    const rect = canvas.getBoundingClientRect()
    const sx = e.clientX - rect.left
    const sy = e.clientY - rect.top
    const { panOffset: pan, zoom: z } = useCanvas2dStore.getState()
    const originScreen = worldToScreen(0, 0, pan, z, canvas.height)
    const dx = sx - originScreen.x
    const dy = sy - originScreen.y
    const type: 'pan' | 'origin' = Math.sqrt(dx * dx + dy * dy) < 10 ? 'origin' : 'pan'
    dragState.current = {
      type,
      startX: sx,
      startY: sy,
      startPanX: pan.x,
      startPanY: pan.y,
      totalMovement: 0,
    }
  }, [])

  const handleMouseMove = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (!dragState.current.type) return
      const canvas = canvasRef.current
      if (!canvas) return
      const rect = canvas.getBoundingClientRect()
      const sx = e.clientX - rect.left
      const sy = e.clientY - rect.top
      const dx = sx - dragState.current.startX
      const dy = sy - dragState.current.startY
      dragState.current.totalMovement = Math.sqrt(dx * dx + dy * dy)

      if (dragState.current.type === 'pan') {
        // dy is negated because worldToScreen inverts Y:
        // screenY = canvasHeight - (worldY * zoom + panOffset.y)
        setPanOffset({
          x: dragState.current.startPanX + dx,
          y: dragState.current.startPanY - dy,
        })
      } else {
        // origin drag — show live preview
        const { panOffset: pan, zoom: z } = useCanvas2dStore.getState()
        const worldPos = screenToWorld(sx, sy, pan, z, canvas.height)
        setOriginDrag({ x: worldPos.x, y: worldPos.y })
      }
    },
    [setPanOffset],
  )

  const handleMouseUp = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (dragState.current.type === 'origin') {
        const canvas = canvasRef.current
        if (canvas) {
          const rect = canvas.getBoundingClientRect()
          const sx = e.clientX - rect.left
          const sy = e.clientY - rect.top
          const { panOffset: pan, zoom: z } = useCanvas2dStore.getState()
          const worldPos = screenToWorld(sx, sy, pan, z, canvas.height)
          onArtworkOriginChange(worldPos.x, worldPos.y)
        }
        setOriginDrag(null)
      }
      dragState.current.type = null
    },
    [onArtworkOriginChange],
  )

  const handleWheel = useCallback(
    (e: React.WheelEvent<HTMLCanvasElement>) => {
      e.preventDefault()
      const canvas = canvasRef.current
      if (!canvas) return
      const rect = canvas.getBoundingClientRect()
      const sx = e.clientX - rect.left
      const sy = e.clientY - rect.top
      const { panOffset: pan, zoom: oldZoom } = useCanvas2dStore.getState()
      const newZoom = Math.min(50.0, Math.max(0.05, oldZoom * (1 - e.deltaY * 0.001)))
      // Keep the world point under the cursor stationary:
      //   screenX = wx * zoom + panX  =>  wx = (sx - panX) / zoom
      //   new panX = sx - wx * newZoom
      const wx = (sx - pan.x) / oldZoom
      const wy = (canvas.height - sy - pan.y) / oldZoom
      setZoom(newZoom)
      setPanOffset({ x: sx - wx * newZoom, y: canvas.height - sy - wy * newZoom })
    },
    [setZoom, setPanOffset],
  )

  const handleClick = useCallback(
    (e: React.MouseEvent<HTMLCanvasElement>) => {
      if (dragState.current.totalMovement >= 5) return
      const canvas = canvasRef.current
      if (!canvas) return
      const rect = canvas.getBoundingClientRect()
      const sx = e.clientX - rect.left
      const sy = e.clientY - rect.top
      const { panOffset: pan, zoom: z } = useCanvas2dStore.getState()

      let nearestId: string | null = null
      let nearestDist = 10 // px threshold

      for (const curve of curves) {
        if (!curve.isClosed) continue
        const rawPoints = fullCurvePoints.get(curve.id)
        if (!rawPoints || rawPoints.length < 2) continue

        const screenPts = rawPoints.map((pt) =>
          worldToScreen(pt[0] + artworkOffset[0], pt[1] + artworkOffset[1], pan, z, canvas.height),
        )

        // Check every edge in the closed loop
        for (let i = 0; i < screenPts.length; i++) {
          const a = screenPts[i]
          const b = screenPts[(i + 1) % screenPts.length]
          const d = distToSegment(sx, sy, a.x, a.y, b.x, b.y)
          if (d < nearestDist) {
            nearestDist = d
            nearestId = curve.id
          }
        }
      }

      setSelectedCurveId(nearestId)
      onCurveSelect(nearestId)
    },
    [curves, fullCurvePoints, artworkOffset, setSelectedCurveId, onCurveSelect],
  )

  // ── Render ───────────────────────────────────────────────────────────────────

  return (
    <div ref={containerRef} style={{ width: '100%', height: '100%' }}>
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onWheel={handleWheel}
        onClick={handleClick}
        style={{ display: 'block', cursor: 'crosshair' }}
      />
    </div>
  )
}
