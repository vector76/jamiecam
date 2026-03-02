import { useState, useEffect } from 'react'
import { useStock, usePushNotification, useProjectStore } from '../../store/projectStore'
import { setStock } from '../../api/stock'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { BoxStock } from '../../api/types'

export function StockPanel() {
  const stock = useStock()
  const pushNotification = usePushNotification()
  const setSnapshot = useProjectStore((s) => s.setSnapshot)

  const [originX, setOriginX] = useState(() => stock?.origin.x ?? 0)
  const [originY, setOriginY] = useState(() => stock?.origin.y ?? 0)
  const [originZ, setOriginZ] = useState(() => stock?.origin.z ?? 0)
  const [width, setWidth] = useState(() => stock?.width ?? 0)
  const [depth, setDepth] = useState(() => stock?.depth ?? 0)
  const [height, setHeight] = useState(() => stock?.height ?? 0)

  useEffect(() => {
    setOriginX(stock?.origin.x ?? 0)
    setOriginY(stock?.origin.y ?? 0)
    setOriginZ(stock?.origin.z ?? 0)
    setWidth(stock?.width ?? 0)
    setDepth(stock?.depth ?? 0)
    setHeight(stock?.height ?? 0)
  }, [stock])

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  async function handleSetStock() {
    const payload: BoxStock = {
      type: 'box',
      origin: { x: originX, y: originY, z: originZ },
      width,
      depth,
      height,
    }
    try {
      await setStock(payload)
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) { handleError(e) }
  }

  async function handleClearStock() {
    try {
      await setStock(null)
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) { handleError(e) }
  }

  return (
    <div>
      {stock == null ? (
        <p>No stock defined</p>
      ) : (
        <div>
          <p>Origin: ({stock.origin.x}, {stock.origin.y}, {stock.origin.z})</p>
          <p>Width: {stock.width} mm, Depth: {stock.depth} mm, Height: {stock.height} mm</p>
        </div>
      )}
      <div>
        <label>
          Origin X (mm)
          <input
            type="number"
            value={originX}
            onChange={(e) => setOriginX(parseFloat(e.target.value) || 0)}
          />
        </label>
        <label>
          Origin Y (mm)
          <input
            type="number"
            value={originY}
            onChange={(e) => setOriginY(parseFloat(e.target.value) || 0)}
          />
        </label>
        <label>
          Origin Z (mm)
          <input
            type="number"
            value={originZ}
            onChange={(e) => setOriginZ(parseFloat(e.target.value) || 0)}
          />
        </label>
        <label>
          Width (X, mm)
          <input
            type="number"
            value={width}
            onChange={(e) => setWidth(parseFloat(e.target.value) || 0)}
          />
        </label>
        <label>
          Depth (Y, mm)
          <input
            type="number"
            value={depth}
            onChange={(e) => setDepth(parseFloat(e.target.value) || 0)}
          />
        </label>
        <label>
          Height (Z, mm)
          <input
            type="number"
            value={height}
            onChange={(e) => setHeight(parseFloat(e.target.value) || 0)}
          />
        </label>
      </div>
      <button onClick={() => void handleSetStock()}>Set Stock</button>
      {stock != null && (
        <button onClick={() => void handleClearStock()}>Clear Stock</button>
      )}
    </div>
  )
}
