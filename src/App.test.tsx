import { render, screen } from '@testing-library/react'
import App from './App'

describe('App', () => {
  it('renders the application title', () => {
    render(<App />)
    expect(screen.getByRole('heading', { name: /jamiecam/i })).toBeInTheDocument()
  })

  it('renders the placeholder description', () => {
    render(<App />)
    expect(screen.getByText(/scaffold placeholder/i)).toBeInTheDocument()
  })
})
