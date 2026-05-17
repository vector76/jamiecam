/**
 * Canvas2DViewport — Mode 2 (2D Profile Cuts) viewport shell.
 *
 * Mounts an HTML <canvas>, keeps it DPR-correct via ResizeObserver, and
 * exposes a small imperative drawing API (polyline, polygon, clear)
 * keyed by a small palette of style tokens (artwork / toolpath / rapid).
 *
 * World coordinates are mapped to screen space through the world→screen
 * affine held in `useViewport2DStore`. The component subscribes to that
 * store so its internal transform ref always reflects current pan/zoom,
 * and it wires pointer-drag pan + wheel-zoom-at-cursor handlers that
 * mutate the store. Stroke widths are interpreted in screen pixels — we
 * apply the world→screen mapping per-vertex inside the drawing API and
 * leave `ctx.lineWidth` untouched by the world transform.
 */

import { useEffect, useImperativeHandle, useRef, type Ref } from 'react'
import { cn } from '@/lib/utils'
import { useViewport2DStore, worldToScreen, type Transform2D } from '../store/viewport2dStore'

export type Canvas2DStyleToken = 'artwork' | 'toolpath' | 'rapid'

interface StyleSpec {
  stroke: string
  lineWidth: number
}

// eslint-disable-next-line react-refresh/only-export-components
export const CANVAS_2D_STYLES: Record<Canvas2DStyleToken, StyleSpec> = {
  artwork: { stroke: '#94a3b8', lineWidth: 1 },
  toolpath: { stroke: '#16a34a', lineWidth: 1.5 },
  rapid: { stroke: '#dc2626', lineWidth: 1 },
}

export interface Canvas2DDrawAPI {
  /** Erase the entire backing store. */
  clear(): void
  /** Draw an open polyline through the given world-space points. */
  polyline(points: ReadonlyArray<readonly [number, number]>, style: Canvas2DStyleToken): void
  /** Draw a closed polygon through the given world-space points. */
  polygon(points: ReadonlyArray<readonly [number, number]>, style: Canvas2DStyleToken): void
}

export interface Canvas2DViewportProps {
  className?: string
  ref?: Ref<Canvas2DDrawAPI>
}

/** Zoom multiplier per single wheel tick. */
const WHEEL_ZOOM_STEP = 1.1

export function Canvas2DViewport({ className, ref }: Canvas2DViewportProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null)
  const transformRef = useRef<Transform2D>(useViewport2DStore.getState().transform)

  useImperativeHandle(
    ref,
    () => ({
      clear() {
        const ctx = ctxRef.current
        const canvas = canvasRef.current
        if (!ctx || !canvas) return
        ctx.save()
        ctx.setTransform(1, 0, 0, 1, 0, 0)
        ctx.clearRect(0, 0, canvas.width, canvas.height)
        ctx.restore()
      },
      polyline(points, style) {
        const ctx = ctxRef.current
        if (!ctx || points.length === 0) return
        const t = transformRef.current
        const s = CANVAS_2D_STYLES[style]
        ctx.beginPath()
        const p0 = worldToScreen(t, points[0][0], points[0][1])
        ctx.moveTo(p0.x, p0.y)
        for (let i = 1; i < points.length; i++) {
          const p = worldToScreen(t, points[i][0], points[i][1])
          ctx.lineTo(p.x, p.y)
        }
        ctx.strokeStyle = s.stroke
        ctx.lineWidth = s.lineWidth
        ctx.stroke()
      },
      polygon(points, style) {
        const ctx = ctxRef.current
        if (!ctx || points.length === 0) return
        const t = transformRef.current
        const s = CANVAS_2D_STYLES[style]
        ctx.beginPath()
        const p0 = worldToScreen(t, points[0][0], points[0][1])
        ctx.moveTo(p0.x, p0.y)
        for (let i = 1; i < points.length; i++) {
          const p = worldToScreen(t, points[i][0], points[i][1])
          ctx.lineTo(p.x, p.y)
        }
        ctx.closePath()
        ctx.strokeStyle = s.stroke
        ctx.lineWidth = s.lineWidth
        ctx.stroke()
      },
    }),
    [],
  )

  // Keep the transform ref synced with the store so the imperative draw
  // API always sees the latest pan/zoom without retriggering the effect
  // that owns useImperativeHandle.
  useEffect(() => {
    return useViewport2DStore.subscribe((state) => {
      transformRef.current = state.transform
    })
  }, [])

  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const ctx = canvas.getContext('2d')
    if (!ctx) return
    ctxRef.current = ctx

    function resize() {
      if (!canvas || !container || !ctx) return
      const dpr = window.devicePixelRatio || 1
      const cssWidth = container.clientWidth
      const cssHeight = container.clientHeight
      canvas.width = Math.max(1, Math.round(cssWidth * dpr))
      canvas.height = Math.max(1, Math.round(cssHeight * dpr))
      canvas.style.width = `${cssWidth}px`
      canvas.style.height = `${cssHeight}px`
      // Pre-scale the context so callers can keep working in CSS pixels.
      // The world→screen transform is applied per-vertex inside the draw
      // API, not via this matrix, so stroke widths stay in screen px.
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
    }

    resize()

    const ro = new ResizeObserver(resize)
    ro.observe(container)

    return () => {
      ro.disconnect()
      ctxRef.current = null
    }
  }, [])

  useEffect(() => {
    // TS doesn't carry narrowing of canvasRef.current through nested
    // closures, so we capture a non-null local and assert once at the
    // top of the effect. The cleanup detaches the listeners before the
    // canvas can ever be unmounted, so the assertion is sound.
    const canvas: HTMLCanvasElement | null = canvasRef.current
    if (!canvas) return
    const c = canvas

    let dragging = false
    let lastX = 0
    let lastY = 0

    function onPointerDown(e: PointerEvent) {
      dragging = true
      lastX = e.clientX
      lastY = e.clientY
      if (typeof c.setPointerCapture === 'function') {
        try {
          c.setPointerCapture(e.pointerId)
        } catch {
          // Some environments (jsdom, certain pointer types) reject
          // capture — pan still works without it.
        }
      }
    }
    function onPointerMove(e: PointerEvent) {
      if (!dragging) return
      const dx = e.clientX - lastX
      const dy = e.clientY - lastY
      lastX = e.clientX
      lastY = e.clientY
      useViewport2DStore.getState().pan(dx, dy)
    }
    function onPointerUp(e: PointerEvent) {
      dragging = false
      if (typeof c.releasePointerCapture === 'function') {
        try {
          if (c.hasPointerCapture(e.pointerId)) {
            c.releasePointerCapture(e.pointerId)
          }
        } catch {
          // Ignore — pointer was never captured.
        }
      }
    }
    function onWheel(e: WheelEvent) {
      e.preventDefault()
      const rect = c.getBoundingClientRect()
      const sx = e.clientX - rect.left
      const sy = e.clientY - rect.top
      const factor = e.deltaY < 0 ? WHEEL_ZOOM_STEP : 1 / WHEEL_ZOOM_STEP
      useViewport2DStore.getState().zoomAt(factor, sx, sy)
    }

    c.addEventListener('pointerdown', onPointerDown)
    c.addEventListener('pointermove', onPointerMove)
    c.addEventListener('pointerup', onPointerUp)
    c.addEventListener('pointercancel', onPointerUp)
    c.addEventListener('wheel', onWheel, { passive: false })

    return () => {
      c.removeEventListener('pointerdown', onPointerDown)
      c.removeEventListener('pointermove', onPointerMove)
      c.removeEventListener('pointerup', onPointerUp)
      c.removeEventListener('pointercancel', onPointerUp)
      c.removeEventListener('wheel', onWheel)
    }
  }, [])

  return (
    <div ref={containerRef} className={cn('relative h-full w-full', className)}>
      <canvas ref={canvasRef} className="block h-full w-full" />
    </div>
  )
}
