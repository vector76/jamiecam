/**
 * Notifications — fixed-position toast overlay.
 *
 * Renders one dismissible toast per notification in the store.
 * Each toast auto-dismisses after 5 seconds and can be manually
 * dismissed by clicking the × button.
 */

import { useEffect } from 'react'
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
    <div
      style={{
        position: 'fixed',
        bottom: '16px',
        right: '16px',
        zIndex: 1000,
        display: 'flex',
        flexDirection: 'column',
        gap: '8px',
      }}
    >
      {notifications.map((message, index) => (
        <div
          key={index}
          style={{
            background: '#333',
            color: '#fff',
            padding: '8px 12px',
            borderRadius: '4px',
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            minWidth: '200px',
          }}
        >
          <span style={{ flex: 1 }}>{message}</span>
          <button
            onClick={() => dismissNotification(index)}
            aria-label="Dismiss notification"
            style={{ background: 'none', border: 'none', color: '#fff', cursor: 'pointer', padding: '0' }}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  )
}
