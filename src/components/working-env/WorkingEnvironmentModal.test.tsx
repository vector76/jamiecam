/**
 * Tests for WorkingEnvironmentModal — CRUD over the persisted working
 * environment (setups, tools, availability matrix) plus the active-setup
 * picker. Backed by fake-indexeddb (installed globally in test-setup.ts)
 * so each test gets a fresh DB.
 */

import { render, screen, fireEvent, waitFor, within } from '@testing-library/react'
import { IDBFactory } from 'fake-indexeddb'
import { WorkingEnvironmentModal } from './WorkingEnvironmentModal'
import { __resetDBForTests } from '../../persistence/db'
import { loadActiveSetupId, loadWorkingEnv, saveWorkingEnv } from '../../persistence/workingEnv'
import type { MachineSetup, Tool, WorkingEnvironment } from '../../api/types'

function makeSetup(id: string, name = `Setup ${id}`): MachineSetup {
  return {
    id,
    name,
    workspace: { origin: { x: 0, y: 0, z: 0 }, width: 300, depth: 200, height: 80 },
    kinematics: '3-axis-router',
    postProcessor: 'grbl-1.1',
    safety: { safeZ: 5, rapidFeedRate: 3000 },
  }
}

function makeTool(id: string, name = `Tool ${id}`): Tool {
  return {
    id,
    name,
    diameter: 3.175,
    fluteCount: 2,
    length: 38,
    material: 'carbide',
    recommended: { spindleRpm: 18000, feedRate: 800, plungeRate: 200 },
  }
}

async function seed(env: WorkingEnvironment): Promise<void> {
  await saveWorkingEnv(env)
}

let idCounter = 0

beforeEach(() => {
  globalThis.indexedDB = new IDBFactory()
  __resetDBForTests()
  idCounter = 0
})

function renderModal(onClose: () => void = () => {}) {
  return render(
    <WorkingEnvironmentModal open onClose={onClose} newId={() => `gen-${++idCounter}`} />,
  )
}

describe('WorkingEnvironmentModal', () => {
  it('renders nothing when open=false', () => {
    const { container } = render(<WorkingEnvironmentModal open={false} onClose={() => {}} />)
    expect(container).toBeEmptyDOMElement()
  })

  it('loads the persisted setups and tools on open', async () => {
    await seed({
      setups: [makeSetup('s1', 'Workshop CNC')],
      tools: [makeTool('t1', '6mm flat')],
      availability: [{ setupId: 's1', toolId: 't1' }],
    })

    renderModal()

    expect(await screen.findByDisplayValue('Workshop CNC')).toBeInTheDocument()
    expect(await screen.findByDisplayValue('6mm flat')).toBeInTheDocument()
  })

  it('creates a new setup and persists it', async () => {
    renderModal()

    fireEvent.click(await screen.findByRole('button', { name: /add setup/i }))

    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.setups).toHaveLength(1)
      expect(env.setups[0].id).toBe('gen-1')
    })
  })

  it('renames a setup and persists the new name', async () => {
    await seed({
      setups: [makeSetup('s1', 'Old Name')],
      tools: [],
      availability: [],
    })

    renderModal()

    const input = await screen.findByDisplayValue('Old Name')
    fireEvent.change(input, { target: { value: 'New Name' } })
    fireEvent.blur(input)

    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.setups[0].name).toBe('New Name')
    })
  })

  it('deletes a setup and removes its availability rows', async () => {
    await seed({
      setups: [makeSetup('s1'), makeSetup('s2')],
      tools: [makeTool('t1')],
      availability: [
        { setupId: 's1', toolId: 't1' },
        { setupId: 's2', toolId: 't1' },
      ],
    })

    renderModal()

    const setupRow = (await screen.findByDisplayValue('Setup s1')).closest('li')
    if (!setupRow) throw new Error('row not found')
    fireEvent.click(within(setupRow as HTMLElement).getByRole('button', { name: /delete/i }))

    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.setups).toEqual([makeSetup('s2')])
      expect(env.availability).toEqual([{ setupId: 's2', toolId: 't1' }])
    })
  })

  it('creates, renames, and deletes a tool', async () => {
    renderModal()

    fireEvent.click(await screen.findByRole('button', { name: /add tool/i }))
    const input = await screen.findByDisplayValue(/new tool/i)
    fireEvent.change(input, { target: { value: 'Renamed Tool' } })
    fireEvent.blur(input)

    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.tools.map((t) => t.name)).toEqual(['Renamed Tool'])
    })

    const row = (await screen.findByDisplayValue('Renamed Tool')).closest('li')
    if (!row) throw new Error('row not found')
    fireEvent.click(within(row as HTMLElement).getByRole('button', { name: /delete/i }))

    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.tools).toEqual([])
    })
  })

  it('toggles availability-matrix cells and persists them', async () => {
    await seed({
      setups: [makeSetup('s1'), makeSetup('s2')],
      tools: [makeTool('t1'), makeTool('t2')],
      availability: [{ setupId: 's1', toolId: 't1' }],
    })

    renderModal()

    // Wait for matrix to render (rows = setups, cols = tools).
    const cellS1T2 = await screen.findByRole('checkbox', {
      name: /Setup s1\b.*Tool t2\b/i,
    })
    expect(cellS1T2).not.toBeChecked()
    const cellS1T1 = await screen.findByRole('checkbox', {
      name: /Setup s1\b.*Tool t1\b/i,
    })
    expect(cellS1T1).toBeChecked()

    fireEvent.click(cellS1T2)
    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.availability).toContainEqual({ setupId: 's1', toolId: 't2' })
    })

    fireEvent.click(cellS1T1)
    await waitFor(async () => {
      const env = await loadWorkingEnv()
      expect(env.availability).not.toContainEqual({ setupId: 's1', toolId: 't1' })
    })
  })

  it('picks the active setup and persists the selection', async () => {
    await seed({
      setups: [makeSetup('s1'), makeSetup('s2')],
      tools: [],
      availability: [],
    })

    renderModal()

    const activeRadio = await screen.findByRole('radio', {
      name: /set active.*Setup s2/i,
    })
    fireEvent.click(activeRadio)

    await waitFor(async () => {
      expect(await loadActiveSetupId()).toBe('s2')
    })
  })

  it('clears the active setup id if the active setup is deleted', async () => {
    await seed({
      setups: [makeSetup('s1'), makeSetup('s2')],
      tools: [],
      availability: [],
    })

    renderModal()

    fireEvent.click(await screen.findByRole('radio', { name: /set active.*Setup s1/i }))
    await waitFor(async () => {
      expect(await loadActiveSetupId()).toBe('s1')
    })

    const row = (await screen.findByDisplayValue('Setup s1')).closest('li')
    if (!row) throw new Error('row not found')
    fireEvent.click(within(row as HTMLElement).getByRole('button', { name: /delete/i }))

    await waitFor(async () => {
      expect(await loadActiveSetupId()).toBeNull()
    })
  })

  it('calls onClose when the close button is clicked', async () => {
    const onClose = vi.fn()
    renderModal(onClose)

    fireEvent.click(await screen.findByRole('button', { name: /close/i }))
    expect(onClose).toHaveBeenCalled()
  })
})
