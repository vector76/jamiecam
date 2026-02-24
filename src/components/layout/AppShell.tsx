/**
 * AppShell — root layout component.
 *
 * Places the Toolbar across the top and the 3-D Viewport in the remaining
 * space.  Phase 0: no side panels.
 */

import { Toolbar } from '../toolbar/Toolbar'
import { Viewport } from '../../viewport/Viewport'

export function AppShell() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <Toolbar />
      <Viewport style={{ flex: 1 }} />
    </div>
  )
}
