import { Sparkles } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { ErrorGroup } from '@/types'

export function GroupList({
  groups, truncated, distinct, selectedId, onSelect, onFilter,
}: {
  groups: ErrorGroup[]
  truncated: boolean
  distinct: number
  selectedId: string | null
  onSelect: (id: string) => void
  onFilter: (g: ErrorGroup) => void
}) {
  return (
    <section aria-label="Top error groups" className="flex min-h-0 flex-col gap-2">
      <h2 className="text-sm font-semibold">Top error groups</h2>
      {groups.length === 0 ? (
        <p className="rounded-md border p-3 text-muted-foreground">No ERROR entries found in this file.</p>
      ) : (
        <ul className="flex min-h-0 flex-col gap-2 overflow-auto pr-1">
          {groups.map((g) => (
            <li key={g.id} className={cn('rounded-lg border bg-card p-3', selectedId === g.id && 'border-primary ring-1 ring-primary')}>
              <div className="flex items-start justify-between gap-2">
                <span className="font-mono text-xs min-w-0 leading-snug break-all" title={g.signature}>
                  {g.signature.length > 140 ? g.signature.slice(0, 140) + '…' : g.signature}
                </span>
                <span className="shrink-0 rounded bg-destructive/15 px-1.5 py-0.5 text-xs font-semibold tabular-nums text-destructive">
                  ×{g.count.toLocaleString()}
                </span>
              </div>
              <div className="mt-1 text-xs text-muted-foreground">Lines {g.sampleLineNumbers.join(', ')}</div>
              <div className="mt-2 flex gap-2">
                <Button size="sm" variant="outline" onClick={() => onFilter(g)}>Show lines</Button>
                <Button size="sm" onClick={() => onSelect(g.id)} aria-label={`Analyze with AI: group of ${g.count} occurrences`}>
                  <Sparkles className="size-3.5" /> Analyze with AI
                </Button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {truncated && (
        <p role="note" className="rounded-md border border-warn/50 bg-warn/10 p-2 text-xs text-warn">
          Grouping limit reached ({distinct.toLocaleString()} distinct groups kept). Later new error shapes were not
          grouped; counts for the groups shown are exact.
        </p>
      )}
    </section>
  )
}
