import { render, screen } from '@testing-library/react'
import { ToolEditorWindow } from './ToolEditorWindow'

describe('ToolEditorWindow', () => {
  it('renders a header area', () => {
    render(<ToolEditorWindow />)
    expect(screen.getByTestId('tool-editor-header')).toBeInTheDocument()
  })

  it('renders a main content area', () => {
    render(<ToolEditorWindow />)
    expect(screen.getByTestId('tool-editor-content')).toBeInTheDocument()
  })

  it('applies dark background styling', () => {
    const { container } = render(<ToolEditorWindow />)
    const root = container.firstElementChild!
    expect(root.className).toContain('bg-background')
    expect(root.className).toContain('text-foreground')
  })
})
