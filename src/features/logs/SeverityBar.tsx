import { cn } from '@/lib/utils'
import { SEVERITIES, type Severity } from '@/types'

const STYLE: Record<Severity, string> = {
  ERROR: 'text-destructive',
  WARN: 'text-warn',
  INFO: 'text-primary',
  DEBUG: 'text-ok',
  UNKNOWN: 'text-muted-foreground',
}

export function SeverityBar({
  counts, total, active, onPick,
}: {
  counts: Record<Severity, number>
  total: number
  active: Severity | 'ALL'
  onPick: (s: Severity | 'ALL') => void
}) {
  return (
    <div className="grid grid-cols-2 gap-2 sm:grid-cols-3 xl:grid-cols-6" role="group" aria-label="Severity filter">
      {(['ALL', ...SEVERITIES] as const).map((s) => {
        const n = s === 'ALL' ? total : counts[s]
        return (
          <button
            key={s}
            type="button"
            aria-pressed={active === s}
            onClick={() => onPick(s)}
            className={cn(
              'rounded-lg border bg-card px-3 py-2 text-left transition-colors hover:bg-muted',
              active === s && 'border-primary ring-1 ring-primary',
            )}
          >
            <div className={cn('text-[11px] font-semibold tracking-wide', s === 'ALL' ? 'text-muted-foreground' : STYLE[s])}>
              {s === 'ALL' ? 'TOTAL LINES' : s}
            </div>
            <div className="text-lg font-semibold tabular-nums">{n.toLocaleString()}</div>
          </button>
        )
      })}
    </div>
  )
}
