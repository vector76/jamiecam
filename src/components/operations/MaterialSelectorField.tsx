import { useEffect, useState } from 'react'
import { listMaterials, lookupFeeds } from '../../api/feeds'
import type { FeedEntry, MaterialMeta } from '../../api/types'

interface Props {
  currentMaterialId: string | null | undefined
  toolMaterial: string | null | undefined
  operationCategory: string
  onMaterialChange: (id: string | null) => void
  onFeedsFetched: (entry: FeedEntry) => void
  onFeedsNotFound: () => void
}

export function MaterialSelectorField({
  currentMaterialId,
  toolMaterial,
  operationCategory,
  onMaterialChange,
  onFeedsFetched,
  onFeedsNotFound,
}: Props) {
  const [materials, setMaterials] = useState<MaterialMeta[]>([])
  const [notFound, setNotFound] = useState(false)

  useEffect(() => {
    listMaterials().then(setMaterials).catch(() => {})
  }, [])

  // Re-run lookup when toolMaterial or operationCategory changes and a material is already
  // selected. currentMaterialId/onFeedsFetched/onFeedsNotFound are intentionally excluded:
  // material-change lookups are handled by handleChange to avoid double-calls, and the
  // callbacks are recreated on every parent render (not useCallback-wrapped).
  useEffect(() => {
    if (!currentMaterialId || !toolMaterial) return
    lookupFeeds(currentMaterialId, toolMaterial, operationCategory)
      .then((entry) => {
        setNotFound(false)
        onFeedsFetched(entry)
      })
      .catch((err: { kind: string; message?: string }) => {
        if (err.kind === 'NotFound') {
          setNotFound(true)
          onFeedsNotFound()
        }
      })
  }, [toolMaterial, operationCategory]) // eslint-disable-line react-hooks/exhaustive-deps

  function handleChange(e: React.ChangeEvent<HTMLSelectElement>) {
    const value = e.target.value
    setNotFound(false)

    if (!value) {
      onMaterialChange(null)
      return
    }

    onMaterialChange(value)

    if (!toolMaterial) return

    lookupFeeds(value, toolMaterial, operationCategory)
      .then((entry) => {
        onFeedsFetched(entry)
      })
      .catch((err: { kind: string; message?: string }) => {
        if (err.kind === 'NotFound') {
          setNotFound(true)
          onFeedsNotFound()
        }
      })
  }

  return (
    <div>
      <select value={currentMaterialId ?? ''} onChange={handleChange}>
        <option value="">-- Select material --</option>
        {materials.map((m) => (
          <option key={m.id} value={m.id}>
            {m.displayName}
          </option>
        ))}
      </select>
      {notFound && (
        <span className="material-not-found-notice">
          No feeds/speeds found for this combination.
        </span>
      )}
    </div>
  )
}
