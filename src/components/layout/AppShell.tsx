/**
 * AppShell — root layout component.
 *
 * Places the Toolbar across the top, the 3-D Viewport in the main area,
 * and the OperationListPanel as a fixed-width sidebar on the right.
 */

import { Toolbar } from '../toolbar/Toolbar'
import { Viewport } from '../../viewport/Viewport'
import { OperationListPanel } from '../operations/OperationListPanel'
import { Notifications } from '../common/Notifications'

export function AppShell() {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100vh' }}>
      <Toolbar />
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <Viewport style={{ flex: 1 }} />
        <OperationListPanel />
      </div>
      <Notifications />
    </div>
  )
}
