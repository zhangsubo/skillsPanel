import * as React from 'react'
import { Check } from 'lucide-react'
import { cn } from '@/lib/utils'

export interface CheckboxProps extends React.InputHTMLAttributes<HTMLInputElement> {
  checked?: boolean
  onCheckedChange?: (checked: boolean) => void
}

const Checkbox = React.forwardRef<HTMLInputElement, CheckboxProps>(
  ({ className, checked, onCheckedChange, ...props }, ref) => {
    return (
      <label className={cn('inline-flex cursor-pointer items-center', className)}>
        <input
          type="checkbox"
          ref={ref}
          checked={checked}
          onChange={(e) => onCheckedChange?.(e.target.checked)}
          className="peer sr-only"
          {...props}
        />
        <span
          className={cn(
            'flex h-4 w-4 items-center justify-center rounded border border-gray-300 bg-white transition-colors',
            'peer-focus-visible:ring-2 peer-focus-visible:ring-ring peer-focus-visible:ring-offset-2',
            'peer-checked:border-primary peer-checked:bg-primary',
            'dark:border-gray-600 dark:bg-gray-800',
            'disabled:cursor-not-allowed disabled:opacity-50',
          )}
        >
          {checked && (
            <Check className="h-3 w-3 text-white" strokeWidth={3} />
          )}
        </span>
      </label>
    )
  },
)
Checkbox.displayName = 'Checkbox'

export { Checkbox }
