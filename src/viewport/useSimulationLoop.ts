import { useEffect, useRef } from 'react'
import type React from 'react'
import type * as THREE from 'three'
import { useViewportStore } from '../store/viewportStore'
import { buildCumulativeDistances, interpolateAtFraction } from './simulationPoints'
import { createToolMesh, positionToolMesh } from './toolMesh'
import { createHighlightIndicator, positionHighlight } from './simulationHighlight'
import type { SceneManager } from './scene'

export function useSimulationLoop(mgrRef: React.RefObject<SceneManager | null>): void {
  const simulationActive = useViewportStore((s) => s.simulationActive)
  const simulationPaused = useViewportStore((s) => s.simulationPaused)
  const simulationPoints = useViewportStore((s) => s.simulationPoints)
  const simulationPlaybackSpeed = useViewportStore((s) => s.simulationPlaybackSpeed)
  const simulationProgress = useViewportStore((s) => s.simulationProgress)

  const toolMeshRef = useRef<THREE.Group | null>(null)
  const highlightRef = useRef<THREE.Mesh | null>(null)
  const cumDistRef = useRef<number[]>([])
  const totalDistRef = useRef<number>(0)
  const accumulatedDistRef = useRef<number>(0)
  const rafIdRef = useRef<number | null>(null)
  const prevTimestampRef = useRef<number | null>(null)
  // True while the RAF loop is the one that wrote simulationProgress this frame.
  const loopUpdatedProgressRef = useRef<boolean>(false)
  // Keep playback speed readable inside the RAF closure without re-creating the loop.
  const simulationPlaybackSpeedRef = useRef(simulationPlaybackSpeed)
  useEffect(() => { simulationPlaybackSpeedRef.current = simulationPlaybackSpeed }, [simulationPlaybackSpeed])

  // ── Mount / unmount ─────────────────────────────────────────────────────────
  useEffect(() => {
    toolMeshRef.current = createToolMesh(6, 20)
    highlightRef.current = createHighlightIndicator()

    return () => {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current)
        rafIdRef.current = null
      }
    }
  }, [])

  // ── Scene membership: add/remove objects when simulation starts/stops ───────
  useEffect(() => {
    const mgr = mgrRef.current
    if (!mgr) return

    if (simulationActive && simulationPoints && simulationPoints.length > 0) {
      const cumDist = buildCumulativeDistances(simulationPoints)
      cumDistRef.current = cumDist
      totalDistRef.current = cumDist[cumDist.length - 1]
      accumulatedDistRef.current = 0
      prevTimestampRef.current = null
      if (toolMeshRef.current) mgr.scene.add(toolMeshRef.current)
      if (highlightRef.current) mgr.scene.add(highlightRef.current)
    } else if (!simulationActive) {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current)
        rafIdRef.current = null
      }
      if (toolMeshRef.current) mgr.scene.remove(toolMeshRef.current)
      if (highlightRef.current) mgr.scene.remove(highlightRef.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [simulationActive, simulationPoints])

  // ── RAF loop: run when active and not paused ─────────────────────────────────
  useEffect(() => {
    if (!simulationActive || simulationPaused) {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current)
        rafIdRef.current = null
      }
      prevTimestampRef.current = null
      return
    }

    function loop(timestamp: number): void {
      const { simulationPoints: pts, stopSimulation, setSimulationProgress } =
        useViewportStore.getState()
      if (!pts || pts.length === 0) return

      if (prevTimestampRef.current !== null) {
        const deltaMs = timestamp - prevTimestampRef.current
        accumulatedDistRef.current += (deltaMs / 1000) * 50 * simulationPlaybackSpeedRef.current
      }
      prevTimestampRef.current = timestamp

      const totalDist = totalDistRef.current
      const fraction = totalDist > 0 ? Math.min(accumulatedDistRef.current / totalDist, 1.0) : 1.0
      const interp = interpolateAtFraction(pts, cumDistRef.current, fraction)

      if (toolMeshRef.current) positionToolMesh(toolMeshRef.current, interp)
      if (highlightRef.current) positionHighlight(highlightRef.current, interp)

      loopUpdatedProgressRef.current = true
      setSimulationProgress(fraction)

      if (fraction >= 1) {
        stopSimulation()
        return
      }

      rafIdRef.current = requestAnimationFrame(loop)
    }

    rafIdRef.current = requestAnimationFrame(loop)

    return () => {
      if (rafIdRef.current !== null) {
        cancelAnimationFrame(rafIdRef.current)
        rafIdRef.current = null
      }
    }
  }, [simulationActive, simulationPaused])

  // ── Scrub: external write to simulationProgress resyncs accumulatedDist ─────
  useEffect(() => {
    if (loopUpdatedProgressRef.current) {
      loopUpdatedProgressRef.current = false
      return
    }
    if (!simulationActive) return

    accumulatedDistRef.current = simulationProgress * totalDistRef.current

    // When paused the RAF loop is not running, so reposition the tool here.
    if (simulationPaused) {
      const pts = useViewportStore.getState().simulationPoints
      if (pts && pts.length > 0) {
        const interp = interpolateAtFraction(pts, cumDistRef.current, simulationProgress)
        if (toolMeshRef.current) positionToolMesh(toolMeshRef.current, interp)
        if (highlightRef.current) positionHighlight(highlightRef.current, interp)
      }
    }
  }, [simulationProgress, simulationActive, simulationPaused])
}
