import { describe, it, expect } from 'vitest'
import { computeWindowTitle } from './windowTitle'

describe('computeWindowTitle', () => {
  it('returns "Untitled — JamieCam" when filePath is null and not dirty', () => {
    expect(computeWindowTitle(null, false)).toBe('Untitled \u2014 JamieCam')
  })

  it('returns "Untitled* — JamieCam" when filePath is null and dirty', () => {
    expect(computeWindowTitle(null, true)).toBe('Untitled* \u2014 JamieCam')
  })

  it('extracts filename from Unix path', () => {
    expect(computeWindowTitle('/path/to/project.jcam', false)).toBe('project.jcam \u2014 JamieCam')
  })

  it('appends * when dirty with a file path', () => {
    expect(computeWindowTitle('/path/to/project.jcam', true)).toBe('project.jcam* \u2014 JamieCam')
  })

  it('extracts filename from Windows-style path', () => {
    expect(computeWindowTitle('C:\\Users\\foo\\bar.jcam', false)).toBe('bar.jcam \u2014 JamieCam')
  })
})
