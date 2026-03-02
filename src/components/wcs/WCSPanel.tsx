import { useState, useEffect } from 'react'
import { useWcs, usePushNotification, useProjectStore } from '../../store/projectStore'
import { setWcs } from '../../api/stock'
import { getProjectSnapshot } from '../../api/file'
import { toAppError } from '../../api/errors'
import type { WorkCoordinateSystem } from '../../api/types'

export function WCSPanel() {
  const wcs = useWcs()
  const pushNotification = usePushNotification()
  const setSnapshot = useProjectStore((s) => s.setSnapshot)

  const [originX, setOriginX] = useState(() => wcs[0]?.origin.x ?? 0)
  const [originY, setOriginY] = useState(() => wcs[0]?.origin.y ?? 0)
  const [originZ, setOriginZ] = useState(() => wcs[0]?.origin.z ?? 0)

  useEffect(() => {
    setOriginX(wcs[0]?.origin.x ?? 0)
    setOriginY(wcs[0]?.origin.y ?? 0)
    setOriginZ(wcs[0]?.origin.z ?? 0)
  }, [wcs])

  function handleError(e: unknown) {
    const err = toAppError(e)
    pushNotification(err.message ?? err.kind ?? 'An error occurred')
  }

  async function handleSetWcs() {
    const payload: WorkCoordinateSystem = wcs[0]
      ? { ...wcs[0], origin: { x: originX, y: originY, z: originZ } }
      : {
          id: crypto.randomUUID(),
          name: 'G54',
          xAxis: { x: 1, y: 0, z: 0 },
          zAxis: { x: 0, y: 0, z: 1 },
          origin: { x: originX, y: originY, z: originZ },
        }
    try {
      await setWcs([payload])
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) { handleError(e) }
  }

  async function handleClearWcs() {
    try {
      await setWcs([])
      const snap = await getProjectSnapshot()
      setSnapshot(snap)
    } catch (e) { handleError(e) }
  }

  return (
    <div>
      {wcs.length === 0 ? (
        <p>No WCS defined</p>
      ) : (
        <div>
          <p>Origin: ({wcs[0].origin.x}, {wcs[0].origin.y}, {wcs[0].origin.z})</p>
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
      </div>
      <button onClick={() => void handleSetWcs()}>Set WCS</button>
      {wcs.length > 0 && (
        <button onClick={() => void handleClearWcs()}>Clear WCS</button>
      )}
    </div>
  )
}
