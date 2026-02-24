/**
 * Smoke test for App.tsx.
 *
 * AppShell (and therefore Viewport + SceneManager) is mocked so these tests
 * stay fast and avoid WebGL dependencies.
 */

import { render, screen } from '@testing-library/react'
import App from './App'

vi.mock('./components/layout/AppShell', () => ({
  AppShell: () => <div data-testid="app-shell">AppShell</div>,
}))

describe('App', () => {
  it('renders the AppShell', () => {
    render(<App />)
    expect(screen.getByTestId('app-shell')).toBeInTheDocument()
  })
})
