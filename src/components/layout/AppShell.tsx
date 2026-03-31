/**
 * AppShell — root layout component.
 *
 * Places the 3-D Viewport in the main area and sidebar panels in a
 * scrollable column on the right.
 */

import { Viewport } from '../../viewport/Viewport'
import { OperationListPanel } from '../operations/OperationListPanel'
import { Notifications } from '../common/Notifications'
import { UnsavedChangesDialog } from '../common/UnsavedChangesDialog'
import { GCodePreviewPanel } from '../gcode/GCodePreviewPanel'
import { StockPanel } from '../stock/StockPanel'
import { WCSPanel } from '../wcs/WCSPanel'
import { SidebarSection } from '@/components/ui/sidebar-section'
import { ScrollArea } from '@/components/ui/scroll-area'

export function AppShell() {
  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <div className="flex flex-1 overflow-hidden">
        <Viewport className="flex-1" />
        <aside className="w-[280px] shrink-0 border-l border-border">
          <ScrollArea className="h-full">
            <SidebarSection title="Stock">
              <StockPanel />
            </SidebarSection>
            <SidebarSection title="WCS" defaultOpen={false}>
              <WCSPanel />
            </SidebarSection>
            <SidebarSection title="Operations">
              <OperationListPanel />
            </SidebarSection>
            <SidebarSection title="G-Code" defaultOpen={false}>
              <GCodePreviewPanel />
            </SidebarSection>
          </ScrollArea>
        </aside>
      </div>
      <Notifications />
      <UnsavedChangesDialog />
    </div>
  )
}
