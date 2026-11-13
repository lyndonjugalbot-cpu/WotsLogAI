import { FolderOpen, Loader2, Settings, XCircle } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { AnalysisPanel } from '@/features/analysis/AnalysisPanel'
import { SettingsDialog } from '@/features/analysis/SettingsDialog'
import { EntryTable } from '@/features/logs/EntryTable'
import { GroupList } from '@/features/logs/GroupList'
import { SearchBar } from '@/features/logs/SearchBar'
import { SeverityBar } from '@/features/logs/SeverityBar'
import { useLogSession } from '@/features/logs/useLogSession'
import { Welcome } from '@/features/logs/Welcome'
import { formatBytes, formatMs, friendlyError, groupFilterQuery } from '@/lib/format'
import { api } from '@/lib/tauri'
import type { AiSettingsView, ErrorGroup } from '@/types'

export default function App() {
  const { state, openFile, openDemo, cancel, setQuery, setSeverity, setPage, selectGroup, dismissError } = useLogSession()
  const [settings, setSettings] = useState<AiSettingsView | null>(null)
  const [settingsOpen, setSettingsOpen] = useState(false)

  useEffect(() => {
    api.getAiSettings().then(setSettings).catch(() => setSettings(null))
  }, [])

  const showLines = useCallback((g: ErrorGroup) => {
    setSeverity('ERROR')
    setQuery(groupFilterQuery(g.signature))
  }, [setSeverity, setQuery])

  const { phase, session, summary, error } = state
  const group = summary?.topErrors.find((g) => g.id === state.selectedGroupId) ?? null

  const settingsDialog = (
    <SettingsDialog open={settingsOpen} settings={settings} onClose={() => setSettingsOpen(false)} onSaved={setSettings} />
  )

  if (phase === 'welcome') {
    return (
      <>
        <Welcome onOpen={openFile} onDemo={openDemo} error={error ? friendlyError(error.code, error.message) : null} />
        <div className="fixed top-3 right-3">
          <Button variant="ghost" size="sm" onClick={() => setSettingsOpen(true)}><Settings className="size-4" /> AI settings</Button>
        </div>
        {settingsDialog}
      </>
    )
  }

  return (
    <div className="flex h-full flex-col gap-3 p-4">
      <header className="flex flex-wrap items-center gap-3">
        <div className="min-w-0">
          <h1 className="truncate text-base font-semibold">
            {session?.displayName}
            {session?.isDemo && <span className="ml-2 rounded border border-warn/50 bg-warn/10 px-1.5 py-0.5 text-[11px] text-warn">DEMO · synthetic data</span>}
          </h1>
          <p className="text-xs text-muted-foreground">
            {session && formatBytes(session.sizeBytes)}
            {summary && ` · ${summary.totalLines.toLocaleString()} lines · scanned in ${formatMs(summary.elapsedMs)}`}
          </p>
        </div>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" size="sm" onClick={openFile}><FolderOpen className="size-4" /> Open file</Button>
          <Button variant="outline" size="sm" onClick={openDemo}>Demo</Button>
          <Button variant="ghost" size="sm" onClick={() => setSettingsOpen(true)}><Settings className="size-4" /> AI settings</Button>
        </div>
      </header>

      {error && phase !== 'error' && (
        <div role="alert" className="flex items-center justify-between rounded-md border border-destructive/50 bg-destructive/10 p-2 text-destructive">
          <span>{friendlyError(error.code, error.message)}</span>
          <div className="flex gap-2">
            {error.code === 'fileChanged' && <Button size="sm" onClick={session?.isDemo ? openDemo : openFile}>Re-import file</Button>}
            <Button size="sm" variant="ghost" onClick={dismissError}>Dismiss</Button>
          </div>
        </div>
      )}

      {phase === 'scanning' && (
        <div role="status" className="rounded-lg border bg-card p-4">
          <div className="flex items-center justify-between">
            <span className="flex items-center gap-2"><Loader2 className="size-4 animate-spin" /> Scanning {session?.displayName}…</span>
            <Button size="sm" variant="outline" onClick={cancel}><XCircle className="size-4" /> Cancel</Button>
          </div>
          <progress className="mt-3 h-2 w-full" aria-label="Scan progress" max={state.progress?.totalBytes || 1} value={state.progress?.bytesRead ?? 0} />
          {state.progress && <p className="mt-1 text-xs text-muted-foreground">{formatBytes(state.progress.bytesRead)} of {formatBytes(state.progress.totalBytes)}</p>}
        </div>
      )}

      {(phase === 'error' || phase === 'cancelled') && (
        <div role="alert" className="rounded-lg border bg-card p-4">
          <p className={phase === 'error' ? 'text-destructive' : 'text-muted-foreground'}>
            {phase === 'cancelled' ? 'Scan cancelled. No results were applied.' : friendlyError(error!.code, error!.message)}
          </p>
          <div className="mt-3 flex gap-2">
            <Button onClick={openFile}>Open another file</Button>
            <Button variant="outline" onClick={openDemo}>Try the demo</Button>
          </div>
        </div>
      )}

      {phase === 'ready' && summary && (
        <div className="grid min-h-0 flex-1 grid-cols-[minmax(280px,22rem)_1fr] gap-3 xl:grid-cols-[minmax(280px,22rem)_1fr_minmax(340px,26rem)]">
          <div className="flex min-h-0 flex-col gap-3 overflow-hidden">
            <GroupList
              groups={summary.topErrors}
              truncated={summary.groupingTruncated}
              distinct={summary.distinctGroups}
              selectedId={state.selectedGroupId}
              onSelect={selectGroup}
              onFilter={showLines}
            />
          </div>
          <div className="flex min-h-0 flex-col gap-3">
            <SeverityBar counts={summary.severityCounts} total={summary.totalLines} active={state.severity} onPick={setSeverity} />
            {(summary.truncatedLines > 0 || summary.invalidUtf8Lines > 0) && (
              <p role="note" className="text-xs text-warn">
                {summary.truncatedLines > 0 && `${summary.truncatedLines} line(s) exceeded 64 KiB and were truncated. `}
                {summary.invalidUtf8Lines > 0 && `${summary.invalidUtf8Lines} line(s) contained invalid UTF-8 and were decoded lossily.`}
              </p>
            )}
            <div className="flex gap-2"><SearchBar value={state.query} onCommit={setQuery} /></div>
            <EntryTable
              data={state.entries}
              loading={state.entriesLoading}
              error={state.entriesError}
              page={state.page}
              onPage={setPage}
              filtered={state.query !== '' || state.severity !== 'ALL'}
            />
          </div>
          {group && session && (
            <div className="col-span-2 min-h-0 xl:col-span-1">
              <AnalysisPanel
                key={group.id + session.sessionId}
                session={session}
                group={group}
                settings={settings}
                onClose={() => selectGroup(null)}
                onOpenSettings={() => setSettingsOpen(true)}
              />
            </div>
          )}
        </div>
      )}
      {settingsDialog}
    </div>
  )
}
