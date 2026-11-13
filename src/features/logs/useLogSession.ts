import { useCallback, useEffect, useRef, useState } from 'react'
import { api, toAppErr } from '@/lib/tauri'
import type { AppErr, PageResult, ScanResult, SessionInfo, Severity } from '@/types'

export type Phase = 'welcome' | 'scanning' | 'ready' | 'cancelled' | 'error'
export const PAGE_SIZE = 100

export interface LogState {
  phase: Phase
  session: SessionInfo | null
  summary: ScanResult | null
  progress: { bytesRead: number; totalBytes: number } | null
  error: AppErr | null
  query: string
  severity: Severity | 'ALL'
  page: number
  entries: PageResult | null
  entriesLoading: boolean
  entriesError: AppErr | null
  selectedGroupId: string | null
}

const initial: LogState = {
  phase: 'welcome', session: null, summary: null, progress: null, error: null,
  query: '', severity: 'ALL', page: 0, entries: null, entriesLoading: false, entriesError: null,
  selectedGroupId: null,
}

/**
 * Owns session/scan/query state. Every async result is checked against the session generation
 * and the latest query id, so results of a previous file or a superseded query are dropped
 * (Rust additionally rejects stale session ids).
 */
export function useLogSession() {
  const [state, setState] = useState<LogState>(initial)
  const gen = useRef(0)
  const seq = useRef(0)
  const scanId = useRef<string | null>(null)
  const activeQuery = useRef<string | null>(null)

  useEffect(() => {
    let un: (() => void) | undefined
    let dead = false
    api
      .onProgress((p) => {
        if (p.scanId === scanId.current) setState((s) => ({ ...s, progress: { bytesRead: p.bytesRead, totalBytes: p.totalBytes } }))
      })
      .then((u) => (dead ? u() : (un = u)))
      .catch(() => {})
    return () => {
      dead = true
      un?.()
    }
  }, [])

  const open = useCallback(async (opener: () => Promise<SessionInfo | null>) => {
    let info: SessionInfo | null
    try {
      info = await opener()
    } catch (e) {
      setState((s) => ({ ...s, error: toAppErr(e) }))
      return
    }
    if (!info) return // picker cancelled; keep the current session
    const g = ++gen.current
    if (scanId.current) void api.cancelScan(scanId.current).catch(() => {})
    if (activeQuery.current) void api.cancelScan(activeQuery.current).catch(() => {})
    activeQuery.current = null
    const sid = `s-${++seq.current}`
    scanId.current = sid
    setState({ ...initial, phase: 'scanning', session: info })
    try {
      const r = await api.scanLog(info.sessionId, sid)
      if (g !== gen.current) return
      setState((s) => ({ ...s, phase: 'ready', summary: r, progress: null }))
    } catch (e) {
      if (g !== gen.current) return
      const err = toAppErr(e)
      setState((s) => ({ ...s, phase: err.code === 'cancelled' ? 'cancelled' : 'error', error: err, progress: null }))
    }
  }, [])

  const openFile = useCallback(() => open(api.selectLog), [open])
  const openDemo = useCallback(() => open(api.loadDemo), [open])
  const cancel = useCallback(() => {
    if (scanId.current) void api.cancelScan(scanId.current).catch(() => {})
  }, [])
  const setQuery = useCallback((query: string) => setState((s) => ({ ...s, query, page: 0 })), [])
  const setSeverity = useCallback((severity: Severity | 'ALL') => setState((s) => ({ ...s, severity, page: 0 })), [])
  const setPage = useCallback((page: number) => setState((s) => ({ ...s, page })), [])
  const selectGroup = useCallback((selectedGroupId: string | null) => setState((s) => ({ ...s, selectedGroupId })), [])
  const dismissError = useCallback(() => setState((s) => ({ ...s, error: null })), [])

  const ready = state.phase === 'ready' && state.session !== null
  const sessionId = state.session?.sessionId
  const { query, severity, page } = state
  useEffect(() => {
    if (!ready || !sessionId) return
    const g = gen.current
    const id = `q-${++seq.current}`
    if (activeQuery.current) void api.cancelScan(activeQuery.current).catch(() => {})
    activeQuery.current = id
    setState((s) => ({ ...s, entriesLoading: true, entriesError: null }))
    api
      .queryLogs({
        sessionId, query, severity: severity === 'ALL' ? null : severity,
        offset: page * PAGE_SIZE, limit: PAGE_SIZE, scanId: id,
      })
      .then((r) => {
        if (activeQuery.current !== id || g !== gen.current) return
        setState((s) => ({ ...s, entries: r, entriesLoading: false }))
      })
      .catch((e) => {
        if (activeQuery.current !== id || g !== gen.current) return
        const err = toAppErr(e)
        if (err.code === 'cancelled') return
        setState((s) => ({ ...s, entriesLoading: false, entriesError: err, error: err.code === 'fileChanged' ? err : s.error }))
      })
  }, [ready, sessionId, query, severity, page])

  return { state, openFile, openDemo, cancel, setQuery, setSeverity, setPage, selectGroup, dismissError }
}
