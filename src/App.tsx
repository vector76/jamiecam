import { useCurrentView } from './store/projectStore'
import { ModeSelector } from './components/modes/ModeSelector'
import { ModePlaceholder } from './components/modes/ModePlaceholder'
import { Notifications } from './components/common/Notifications'
import { UnsavedChangesDialog } from './components/common/UnsavedChangesDialog'

export default function App() {
  const view = useCurrentView()

  return (
    <div>
      <Notifications />
      <UnsavedChangesDialog />
      {view === 'selector' ? <ModeSelector /> : <ModePlaceholder mode={view} />}
    </div>
  )
}
