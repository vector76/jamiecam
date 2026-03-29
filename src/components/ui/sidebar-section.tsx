import { useId, useState } from 'react'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'

interface SidebarSectionProps {
  title: string
  defaultOpen?: boolean
  action?: React.ReactNode
  children: React.ReactNode
}

export function SidebarSection({
  title,
  defaultOpen = true,
  action,
  children,
}: SidebarSectionProps) {
  const [open, setOpen] = useState(defaultOpen)
  const contentId = useId()

  return (
    <div className="border-b border-border">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        aria-expanded={open}
        aria-controls={contentId}
        className="flex w-full items-center gap-1 px-3 py-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground hover:bg-accent"
      >
        <ChevronDown
          aria-hidden="true"
          className={cn('h-3.5 w-3.5 transition-transform', !open && '-rotate-90')}
        />
        <span className="flex-1 text-left">{title}</span>
        {action && (
          <span onClick={(e) => e.stopPropagation()} className="ml-auto">
            {action}
          </span>
        )}
      </button>
      {open && (
        <div id={contentId} className="px-3 pb-3 pt-1">
          {children}
        </div>
      )}
    </div>
  )
}
