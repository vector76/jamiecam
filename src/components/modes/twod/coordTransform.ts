import type { CurveSummary } from '../../../api/twodMode'

/**
 * Convert a world-space point to canvas screen coordinates.
 *
 * The Y-axis is inverted: world Y=0 maps to the bottom of the canvas,
 * world Y increases upward, screen Y increases downward.
 *
 * screenX = worldX * zoom + panOffset.x
 * screenY = canvasHeight - (worldY * zoom + panOffset.y)
 */
export function worldToScreen(
  wx: number,
  wy: number,
  panOffset: { x: number; y: number },
  zoom: number,
  canvasHeight: number,
): { x: number; y: number } {
  return {
    x: wx * zoom + panOffset.x,
    y: canvasHeight - (wy * zoom + panOffset.y),
  }
}

/**
 * Convert canvas screen coordinates back to world space.
 *
 * Inverse of worldToScreen.
 */
export function screenToWorld(
  sx: number,
  sy: number,
  panOffset: { x: number; y: number },
  zoom: number,
  canvasHeight: number,
): { x: number; y: number } {
  return {
    x: (sx - panOffset.x) / zoom,
    y: (canvasHeight - sy - panOffset.y) / zoom,
  }
}

/**
 * Compute a pan offset and zoom level that fit all curve bounding boxes
 * (and optionally the stock dimensions) within the given canvas area.
 *
 * @param curves         Curve summaries whose bboxes should be visible.
 * @param stockDims      Stock width/depth, or null if unavailable.
 * @param canvasWidth    Canvas width in pixels.
 * @param canvasHeight   Canvas height in pixels.
 * @param paddingFraction Fraction of the smallest canvas dimension kept as
 *                        padding on each side (default 0.05 = 5%).
 */
export function autoFitTransform(
  curves: CurveSummary[],
  stockDims: { width: number; depth: number } | null,
  canvasWidth: number,
  canvasHeight: number,
  paddingFraction = 0.05,
): { panOffset: { x: number; y: number }; zoom: number } {
  // Collect world-space bounds from curves and optional stock rectangle.
  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity

  for (const c of curves) {
    minX = Math.min(minX, c.bbox.minX)
    minY = Math.min(minY, c.bbox.minY)
    maxX = Math.max(maxX, c.bbox.maxX)
    maxY = Math.max(maxY, c.bbox.maxY)
  }

  if (stockDims) {
    minX = Math.min(minX, 0)
    minY = Math.min(minY, 0)
    maxX = Math.max(maxX, stockDims.width)
    maxY = Math.max(maxY, stockDims.depth)
  }

  // Fallback when there is nothing to fit.
  if (!isFinite(minX) || !isFinite(minY) || !isFinite(maxX) || !isFinite(maxY)) {
    return { panOffset: { x: 0, y: 0 }, zoom: 1.0 }
  }

  const worldW = maxX - minX
  const worldH = maxY - minY

  const padding = paddingFraction * Math.min(canvasWidth, canvasHeight)
  const availW = canvasWidth - 2 * padding
  const availH = canvasHeight - 2 * padding

  // Choose the zoom that fits both dimensions; guard against degenerate extents.
  let zoom = 1.0
  if (worldW > 0 && worldH > 0) {
    zoom = Math.min(availW / worldW, availH / worldH)
  } else if (worldW > 0) {
    zoom = availW / worldW
  } else if (worldH > 0) {
    zoom = availH / worldH
  }

  // Centre the content in the canvas.
  // worldToScreen: screenX = wx * zoom + panOffset.x
  //                screenY = canvasHeight - (wy * zoom + panOffset.y)
  // We want the centre of [minX..maxX] to map to canvasWidth/2:
  //   (minX + maxX) / 2 * zoom + panOffset.x = canvasWidth / 2
  // => panOffset.x = canvasWidth / 2 - (minX + maxX) / 2 * zoom
  //
  // We want the centre of [minY..maxY] to map to canvasHeight/2:
  //   canvasHeight - ((minY + maxY) / 2 * zoom + panOffset.y) = canvasHeight / 2
  // => panOffset.y = canvasHeight / 2 - (minY + maxY) / 2 * zoom
  const cx = (minX + maxX) / 2
  const cy = (minY + maxY) / 2

  return {
    panOffset: {
      x: canvasWidth / 2 - cx * zoom,
      y: canvasHeight / 2 - cy * zoom,
    },
    zoom,
  }
}
