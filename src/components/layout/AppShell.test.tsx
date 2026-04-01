/**
 * Tests for AppShell.tsx — root layout component.
 *
 * All child components are mocked to keep tests fast and avoid WebGL
 * dependencies.
 */

import { render, screen } from '@testing-library/react'
import { AppShell } from './AppShell'

// ── Module mocks ─────────────────────────────────────────────────────────────

vi.mock('../../viewport/Viewport', () => ({
  Viewport: (props: { className?: string }) => (
    <div data-testid="viewport" className={props.className}>
      Viewport
    </div>
  ),
}))

vi.mock('../operations/OperationListPanel', () => ({
  OperationListPanel: () => <div data-testid="operation-list-panel">OperationListPanel</div>,
}))

vi.mock('../common/Notifications', () => ({
  Notifications: () => <div data-testid="notifications">Notifications</div>,
}))

vi.mock('../common/UnsavedChangesDialog', () => ({
  UnsavedChangesDialog: () => <div data-testid="unsaved-changes-dialog">UnsavedChangesDialog</div>,
}))

vi.mock('../gcode/GCodePreviewPanel', () => ({
  GCodePreviewPanel: () => <div data-testid="gcode-preview-panel">GCodePreviewPanel</div>,
}))

vi.mock('../stock/StockPanel', () => ({
  StockPanel: () => <div data-testid="stock-panel">StockPanel</div>,
}))

vi.mock('../wcs/WCSPanel', () => ({
  WCSPanel: () => <div data-testid="wcs-panel">WCSPanel</div>,
}))

vi.mock('../simulation/MaterialRemovalPanel', () => ({
  MaterialRemovalPanel: () => (
    <div data-testid="material-removal-panel">MaterialRemovalPanel</div>
  ),
}))

vi.mock('@/components/ui/sidebar-section', () => ({
  SidebarSection: ({ title, children }: { title: string; children: React.ReactNode }) => (
    <div data-testid={`sidebar-section-${title.toLowerCase()}`}>{children}</div>
  ),
}))

vi.mock('@/components/ui/scroll-area', () => ({
  ScrollArea: ({ children, className }: { children: React.ReactNode; className?: string }) => (
    <div data-testid="scroll-area" className={className}>{children}</div>
  ),
}))

// ── Tests ────────────────────────────────────────────────────────────────────

describe('AppShell', () => {
  it('renders the Viewport', () => {
    render(<AppShell />)
    expect(screen.getByTestId('viewport')).toBeInTheDocument()
  })

  it('renders sidebar panels', () => {
    render(<AppShell />)
    expect(screen.getByTestId('stock-panel')).toBeInTheDocument()
    expect(screen.getByTestId('wcs-panel')).toBeInTheDocument()
    expect(screen.getByTestId('operation-list-panel')).toBeInTheDocument()
    expect(screen.getByTestId('gcode-preview-panel')).toBeInTheDocument()
    expect(screen.getByTestId('material-removal-panel')).toBeInTheDocument()
  })

  it('renders Notifications and UnsavedChangesDialog', () => {
    render(<AppShell />)
    expect(screen.getByTestId('notifications')).toBeInTheDocument()
    expect(screen.getByTestId('unsaved-changes-dialog')).toBeInTheDocument()
  })

  it('does not render a Toolbar', () => {
    render(<AppShell />)
    // After Toolbar removal, no toolbar-related elements should be present.
    expect(screen.queryByText('Open Model')).not.toBeInTheDocument()
    expect(screen.queryByText('New Project')).not.toBeInTheDocument()
    expect(screen.queryByText('Save Project')).not.toBeInTheDocument()
  })
})
