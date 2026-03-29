import { Label } from '@/components/ui/label'

interface FormFieldProps {
  label: string
  htmlFor?: string
  children: React.ReactNode
}

export function FormField({ label, htmlFor, children }: FormFieldProps) {
  return (
    <div className="mb-1.5 space-y-0.5">
      <Label htmlFor={htmlFor} className="text-xs text-muted-foreground">
        {label}
      </Label>
      {children}
    </div>
  )
}
