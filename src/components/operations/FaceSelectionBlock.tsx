import { useViewportStore } from '../../store/viewportStore'
import { getModelFaces } from '../../api/geometry'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'

interface Props {
  geometry: string[] | null | undefined
  onSave: (geometry: string[] | null) => void
}

export function FaceSelectionBlock({ geometry, onSave }: Props) {
  const selectionMode = useViewportStore((s) => s.selectionMode)
  const selectedFps = useViewportStore((s) => s.selectedFaceFingerprints)
  const setSelectionMode = useViewportStore((s) => s.setSelectionMode)
  const setFaceDescriptors = useViewportStore((s) => s.setFaceDescriptors)
  const clearFaceSelection = useViewportStore((s) => s.clearFaceSelection)

  async function handleSelectFaces() {
    const faces = await getModelFaces()
    setFaceDescriptors(faces)
    if (geometry?.length) {
      useViewportStore.getState().clearFaceSelection()
      geometry.forEach((fp) => useViewportStore.getState().toggleFaceSelection(fp))
    } else {
      clearFaceSelection()
    }
    setSelectionMode(true)
  }

  function handleDoneSelecting() {
    setSelectionMode(false)
    const fps = useViewportStore.getState().selectedFaceFingerprints
    onSave(fps.length ? fps : null)
  }

  function handleClearGeometry() {
    clearFaceSelection()
    onSave(null)
  }

  return (
    <div className="mt-2 space-y-1.5">
      <Separator />
      <p className="text-xs text-muted-foreground">
        {selectionMode
          ? `${selectedFps.length} face(s) selected`
          : geometry?.length
            ? `${geometry.length} face(s) selected`
            : 'Stock boundary (default)'}
      </p>
      <div className="flex gap-1.5">
        {selectionMode ? (
          <Button variant="outline" size="sm" onClick={() => handleDoneSelecting()}>
            Done Selecting
          </Button>
        ) : (
          <Button variant="outline" size="sm" onClick={() => void handleSelectFaces()}>
            Select Faces
          </Button>
        )}
        {!selectionMode && geometry?.length ? (
          <Button variant="ghost" size="sm" onClick={() => handleClearGeometry()}>
            Clear
          </Button>
        ) : null}
      </div>
    </div>
  )
}
