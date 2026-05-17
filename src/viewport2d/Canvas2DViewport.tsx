/**
 * Canvas2DViewport — Mode 2 (2D Profile Cuts) viewport shell.
 *
 * Mounts an HTML <canvas>, keeps it DPR-correct via ResizeObserver, and
 * exposes a small imperative drawing API (polyline, polygon, clear) keyed
 * by a small palette of style tokens (artwork / toolpath / rapid).
 *
 * World-to-screen is identity for now: callers supply coordinates in CSS
 * pixels and the canvas context is pre-scaled by devicePixelRatio so a
 * world unit equals one CSS pixel. Pan/zoom and a proper world-space
 * transform land in the next bead together with the Mode 2 store.
 */

import { useEffect, useImperativeHandle, useRef, type Ref } from 'react'
import { cn } from '@/lib/utils'

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

export function Canvas2DViewport({ className, ref }: Canvas2DViewportProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const ctxRef = useRef<CanvasRenderingContext2D | null>(null)

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
        const s = CANVAS_2D_STYLES[style]
        ctx.beginPath()
        ctx.moveTo(points[0][0], points[0][1])
        for (let i = 1; i < points.length; i++) {
          ctx.lineTo(points[i][0], points[i][1])
        }
        ctx.strokeStyle = s.stroke
        ctx.lineWidth = s.lineWidth
        ctx.stroke()
      },
      polygon(points, style) {
        const ctx = ctxRef.current
        if (!ctx || points.length === 0) return
        const s = CANVAS_2D_STYLES[style]
        ctx.beginPath()
        ctx.moveTo(points[0][0], points[0][1])
        for (let i = 1; i < points.length; i++) {
          ctx.lineTo(points[i][0], points[i][1])
        }
        ctx.closePath()
        ctx.strokeStyle = s.stroke
        ctx.lineWidth = s.lineWidth
        ctx.stroke()
      },
    }),
    [],
  )

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

  return (
    <div ref={containerRef} className={cn('relative h-full w-full', className)}>
      <canvas ref={canvasRef} className="block h-full w-full" />
    </div>
  )
}
