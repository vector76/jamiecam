import { useState, useEffect } from 'react'
import { useStock, usePushNotification, useProjectStore } from '../../store/projectStore'
import { setStock } from '../../api/stock'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { FormField } from '@/components/ui/form-field'
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
    } catch (e) {
      handleError(e)
    }
  }

  async function handleClearStock() {
    try {
      await setStock(null)
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) {
      handleError(e)
    }
  }

  return (
    <div className="space-y-2">
      {stock == null ? (
        <p className="text-xs text-muted-foreground">No stock defined</p>
      ) : (
        <div className="space-y-0.5 font-mono text-xs text-muted-foreground">
          <p>
            Origin: ({stock.origin.x}, {stock.origin.y}, {stock.origin.z})
          </p>
          <p>
            {stock.width} x {stock.depth} x {stock.height} mm
          </p>
        </div>
      )}
      <div>
        <p className="mb-1 text-xs font-medium text-muted-foreground">Origin</p>
        <div className="grid grid-cols-3 gap-2">
          <FormField label="X (mm)" htmlFor="stock-ox">
            <Input
              id="stock-ox"
              type="number"
              value={originX}
              onChange={(e) => setOriginX(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Y (mm)" htmlFor="stock-oy">
            <Input
              id="stock-oy"
              type="number"
              value={originY}
              onChange={(e) => setOriginY(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Z (mm)" htmlFor="stock-oz">
            <Input
              id="stock-oz"
              type="number"
              value={originZ}
              onChange={(e) => setOriginZ(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
        </div>
      </div>
      <div>
        <p className="mb-1 text-xs font-medium text-muted-foreground">Dimensions</p>
        <div className="grid grid-cols-3 gap-2">
          <FormField label="Width (X)" htmlFor="stock-w">
            <Input
              id="stock-w"
              type="number"
              value={width}
              onChange={(e) => setWidth(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Depth (Y)" htmlFor="stock-d">
            <Input
              id="stock-d"
              type="number"
              value={depth}
              onChange={(e) => setDepth(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
          <FormField label="Height (Z)" htmlFor="stock-h">
            <Input
              id="stock-h"
              type="number"
              value={height}
              onChange={(e) => setHeight(parseFloat(e.target.value) || 0)}
              className="h-7 text-xs"
            />
          </FormField>
        </div>
      </div>
      <div className="flex gap-2">
        <Button variant="outline" size="sm" onClick={() => void handleSetStock()}>
          Set Stock
        </Button>
        {stock != null && (
          <Button variant="ghost" size="sm" onClick={() => void handleClearStock()}>
            Clear
          </Button>
        )}
      </div>
    </div>
  )
}
