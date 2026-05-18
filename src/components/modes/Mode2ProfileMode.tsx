/**
 * Mode2ProfileMode — Phase 4 (2-D Profile Cuts) placeholder.
 *
 * The full Mode 2 surface (SVG/DXF import, profile planner, Canvas2D
 * workspace) lands across later beads. This stub exists so the App
 * shell can already dispatch by `ProjectState.mode` and so opening a
 * `2d-profile` `.jcam` mounts something distinct from Mode 1.
 */

import type { ProjectState } from '../../persistence/projectFile'

interface Mode2ProfileModeProps {
  initialProject?: ProjectState | null
}

export function Mode2ProfileMode(_props: Mode2ProfileModeProps = {}) {
  return (
    <div
      data-testid="mode2-placeholder"
      className="flex h-full flex-1 items-center justify-center bg-background text-foreground"
    >
      <p className="text-sm text-muted-foreground">
        Mode 2: 2-D Profile Cuts — coming soon.
      </p>
    </div>
  )
}
