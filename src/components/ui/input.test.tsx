import { render, screen, fireEvent, createEvent } from '@testing-library/react'
import { Input } from './input'

describe('Input', () => {
  it('prevents default on wheel event for numeric input', () => {
    render(<Input type="number" defaultValue="5" />)
    const input = screen.getByRole('spinbutton')
    const event = createEvent.wheel(input, { bubbles: true, cancelable: true })
    fireEvent(input, event)
    expect(event.defaultPrevented).toBe(true)
  })

  it('does not prevent default on wheel event for text input', () => {
    render(<Input type="text" defaultValue="hello" />)
    const input = screen.getByRole('textbox')
    const event = createEvent.wheel(input, { bubbles: true, cancelable: true })
    fireEvent(input, event)
    expect(event.defaultPrevented).toBe(false)
  })
})
