import { ChevronLeft, ChevronRight } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { AppErr, PageResult, Severity } from '@/types'
import { PAGE_SIZE } from './useLogSession'

const VARIANT = { ERROR: 'error', WARN: 'warn', INFO: 'info', DEBUG: 'debug', UNKNOWN: 'outline' } as const satisfies Record<Severity, string>

export function EntryTable({
  data, loading, error, page, onPage, filtered,
}: {
  data: PageResult | null
  loading: boolean
  error: AppErr | null
  page: number
  onPage: (p: number) => void
  filtered: boolean
}) {
  const total = data?.totalMatches ?? 0
  const pages = Math.max(1, Math.ceil(total / PAGE_SIZE))
  const from = total === 0 ? 0 : page * PAGE_SIZE + 1
  const to = Math.min(total, (page + 1) * PAGE_SIZE)
  return (
    <section aria-label="Log entries" className="flex min-h-0 flex-1 flex-col rounded-lg border bg-card">
      <div className="min-h-0 flex-1 overflow-auto" aria-busy={loading}>
        {error ? (
          <p role="alert" className="p-4 text-destructive">{error.message}</p>
        ) : !data ? (
          <p className="p-4 text-muted-foreground">Loading entries…</p>
        ) : data.entries.length === 0 ? (
          <p className="p-6 text-center text-muted-foreground">
            {filtered ? 'No entries match the current search and filter.' : 'This file has no lines.'}
          </p>
        ) : (
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-muted text-left text-muted-foreground">
              <tr>
                <th className="w-16 px-3 py-2 font-medium">Line</th>
                <th className="w-56 px-3 py-2 font-medium">Time</th>
                <th className="w-24 px-3 py-2 font-medium">Level</th>
                <th className="px-3 py-2 font-medium">Message</th>
              </tr>
            </thead>
            <tbody className={loading ? 'opacity-60' : undefined}>
              {data.entries.map((e) => (
                <tr key={e.lineNumber} className="border-t align-top hover:bg-muted/50">
                  <td className="px-3 py-1.5 font-mono tabular-nums text-muted-foreground">{e.lineNumber}</td>
                  <td className="whitespace-nowrap px-3 py-1.5 font-mono text-muted-foreground">{e.timestamp ?? ''}</td>
                  <td className="px-3 py-1.5"><Badge variant={VARIANT[e.severity]}>{e.severity}</Badge></td>
                  <td className="px-3 py-1.5 font-mono break-all whitespace-pre-wrap">
                    {e.message.length > 600 ? e.message.slice(0, 600) + '…' : e.message}
                    {(e.truncated || e.message.length > 600) && <span className="ml-1 text-warn">[truncated]</span>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
      <div className="flex items-center justify-between border-t px-3 py-2 text-xs text-muted-foreground">
        <span aria-live="polite">
          {total === 0 ? '0 matches' : `${from.toLocaleString()}–${to.toLocaleString()} of ${total.toLocaleString()} matches`}
        </span>
        <div className="flex items-center gap-2">
          <Button size="icon" variant="outline" className="size-7" aria-label="Previous page" disabled={page === 0} onClick={() => onPage(page - 1)}>
            <ChevronLeft className="size-4" />
          </Button>
          <span className="tabular-nums">Page {page + 1} / {pages}</span>
          <Button size="icon" variant="outline" className="size-7" aria-label="Next page" disabled={page + 1 >= pages} onClick={() => onPage(page + 1)}>
            <ChevronRight className="size-4" />
          </Button>
        </div>
      </div>
    </section>
  )
}
