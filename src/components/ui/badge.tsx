import { cva, type VariantProps } from 'class-variance-authority'
import * as React from 'react'
import { cn } from '@/lib/utils'

const badgeVariants = cva('inline-flex items-center rounded border px-1.5 py-0.5 text-[11px] font-semibold tracking-wide', {
  variants: {
    variant: {
      default: 'bg-muted text-foreground',
      error: 'border-destructive/50 bg-destructive/15 text-destructive',
      warn: 'border-warn/50 bg-warn/15 text-warn',
      info: 'border-primary/50 bg-primary/15 text-primary',
      debug: 'border-ok/50 bg-ok/15 text-ok',
      outline: 'bg-transparent text-muted-foreground',
    },
  },
  defaultVariants: { variant: 'default' },
})

export function Badge({
  className,
  variant,
  ...props
}: React.HTMLAttributes<HTMLSpanElement> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />
}
