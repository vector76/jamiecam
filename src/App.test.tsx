/**
 * Tests for App.tsx — the navigation root.
 *
 * Sub-components (ModeSelector, ModePlaceholder, Notifications,
 * UnsavedChangesDialog) are mocked so these tests stay fast and
 * focus on the routing logic driven by useCurrentView.
 */

import { render, screen } from '@testing-library/react'
import { useProjectStore } from './store/projectStore'
import App from './App'
import type { ProjectSnapshot } from './api/types'

vi.mock('./components/modes/ModeSelector', () => ({
  ModeSelector: () => <div data-testid="mode-selector">Select a Mode</div>,
}))

vi.mock('./components/modes/ModePlaceholder', () => ({
  ModePlaceholder: ({ mode }: { mode: string }) => (
    <div data-testid="mode-placeholder" data-mode={mode} />
  ),
}))

vi.mock('./components/common/Notifications', () => ({
  Notifications: () => <div data-testid="notifications" />,
}))

vi.mock('./components/common/UnsavedChangesDialog', () => ({
  UnsavedChangesDialog: () => <div data-testid="unsaved-dialog" />,
}))

const SNAPSHOT: ProjectSnapshot = {
  modelPath: null,
  modelChecksum: null,
  projectName: 'Test Project',
  modifiedAt: '',
  tools: [],
  stock: null,
  wcs: [],
  operations: [],
  projectIsOpen: true,
  filePath: null,
  dirty: false,
  mode: '3d',
}

beforeEach(() => {
  useProjectStore.setState({ snapshot: null })
})

describe('App', () => {
  describe('selector state (snapshot is null)', () => {
    it('renders ModeSelector (contains mode labels)', () => {
      render(<App />)
      expect(screen.getByTestId('mode-selector')).toBeInTheDocument()
      expect(screen.queryByTestId('mode-placeholder')).not.toBeInTheDocument()
    })

    it('renders Notifications', () => {
      render(<App />)
      expect(screen.getByTestId('notifications')).toBeInTheDocument()
    })

    it('renders UnsavedChangesDialog', () => {
      render(<App />)
      expect(screen.getByTestId('unsaved-dialog')).toBeInTheDocument()
    })
  })

  describe('mode state (projectIsOpen: true, mode: "3d")', () => {
    beforeEach(() => {
      useProjectStore.setState({ snapshot: { ...SNAPSHOT, projectIsOpen: true, mode: '3d' } })
    })

    it('renders ModePlaceholder for 3D Surface and not ModeSelector', () => {
      render(<App />)
      expect(screen.queryByTestId('mode-selector')).not.toBeInTheDocument()
      const placeholder = screen.getByTestId('mode-placeholder')
      expect(placeholder).toBeInTheDocument()
      expect(placeholder).toHaveAttribute('data-mode', '3d')
    })

    it('renders Notifications', () => {
      render(<App />)
      expect(screen.getByTestId('notifications')).toBeInTheDocument()
    })

    it('renders UnsavedChangesDialog', () => {
      render(<App />)
      expect(screen.getByTestId('unsaved-dialog')).toBeInTheDocument()
    })
  })
})
