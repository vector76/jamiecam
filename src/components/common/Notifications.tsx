/**
 * Notifications — fixed-position toast overlay.
 *
 * Renders one dismissible toast per notification in the store.
 * Each toast auto-dismisses after 5 seconds and can be manually
 * dismissed by clicking the × button.
 */

import { useEffect } from 'react'
import { X } from 'lucide-react'
import { useNotifications, useProjectStore } from '../../store/projectStore'

export function Notifications() {
  const notifications = useNotifications()
  const dismissNotification = useProjectStore((s) => s.dismissNotification)

  useEffect(() => {
    if (notifications.length === 0) return
    const id = setTimeout(() => dismissNotification(0), 5000)
    return () => clearTimeout(id)
  }, [notifications[0], dismissNotification])

  if (notifications.length === 0) return null

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {notifications.map((message, index) => (
        <div
          key={index}
          className="flex min-w-[200px] items-center gap-2 rounded-md bg-card px-3 py-2 text-sm text-card-foreground shadow-lg"
        >
          <span className="flex-1">{message}</span>
          <button
            onClick={() => dismissNotification(index)}
            aria-label="Dismiss notification"
            className="rounded-sm p-0.5 text-muted-foreground hover:text-foreground"
          >
            <X className="h-3.5 w-3.5" />
          </button>
        </div>
      ))}
    </div>
  )
}
