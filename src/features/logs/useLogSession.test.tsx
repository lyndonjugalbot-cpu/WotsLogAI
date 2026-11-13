import { act, renderHook, waitFor } from '@testing-library/react'
import { beforeEach, vi } from 'vitest'
import type { PageResult, ScanResult, SessionInfo } from '@/types'

const hoisted = vi.hoisted(() => ({ api: {} as Record<string, ReturnType<typeof vi.fn>> }))
vi.mock('@/lib/tauri', () => ({
  api: hoisted.api,
  toAppErr: (e: unknown) => (e && typeof e === 'object' && 'code' in e ? e : { code: 'unknown', message: String(e) }),
}))
import { useLogSession } from './useLogSession'

const info = (id: string): SessionInfo => ({ sessionId: id, displayName: `${id}.log`, sizeBytes: 1, isDemo: false })
const summary = (id: string, total: number): ScanResult => ({
  sessionId: id, scanId: 's', maxFileBytes: 1, totalLines: total, topErrors: [], elapsedMs: 1, groupingTruncated: false,
  distinctGroups: 0, bytes: 1, truncatedLines: 0, invalidUtf8Lines: 0,
  severityCounts: { ERROR: 0, WARN: 0, INFO: total, DEBUG: 0, UNKNOWN: 0 },
})
const page = (n: number, offset = 0): PageResult => ({ sessionId: 'x', scanId: 'q', entries: [], totalMatches: n, offset, limit: 100 })
function deferred<T>() {
  let resolve!: (v: T) => void
  let reject!: (e: unknown) => void
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

beforeEach(() => {
  Object.assign(hoisted.api, {
    selectLog: vi.fn(), loadDemo: vi.fn(), scanLog: vi.fn(), queryLogs: vi.fn().mockResolvedValue(page(0)),
    cancelScan: vi.fn().mockResolvedValue(undefined), onProgress: vi.fn().mockResolvedValue(() => {}),
  })
})

describe('useLogSession stale-result protection', () => {
  it('drops the scan result of a previous file when a new file is opened', async () => {
    const a = deferred<ScanResult>()
    hoisted.api.selectLog.mockResolvedValueOnce(info('A')).mockResolvedValueOnce(info('B'))
    hoisted.api.scanLog.mockImplementationOnce(() => a.promise).mockResolvedValueOnce(summary('B', 22))
    const { result } = renderHook(() => useLogSession())

    act(() => { void result.current.openFile() }) // A: scan stays pending
    await waitFor(() => expect(result.current.state.phase).toBe('scanning'))
    await act(async () => { await result.current.openFile() }) // B finishes
    await waitFor(() => expect(result.current.state.summary?.totalLines).toBe(22))
    await act(async () => { a.resolve(summary('A', 11111)) }) // late A result
    expect(result.current.state.summary?.totalLines).toBe(22)
    expect(result.current.state.session?.sessionId).toBe('B')
    expect(hoisted.api.cancelScan).toHaveBeenCalled()
  })

  it('applies only the latest query result', async () => {
    const slow = deferred<PageResult>()
    hoisted.api.loadDemo.mockResolvedValue(info('D'))
    hoisted.api.scanLog.mockResolvedValue(summary('D', 5))
    hoisted.api.queryLogs.mockReset()
    hoisted.api.queryLogs.mockImplementationOnce(() => Promise.resolve(page(5))) // initial
    hoisted.api.queryLogs.mockImplementationOnce(() => slow.promise) // "foo"
    hoisted.api.queryLogs.mockImplementationOnce(() => Promise.resolve(page(2))) // "foobar"
    const { result } = renderHook(() => useLogSession())
    await act(async () => { await result.current.openDemo() })
    await waitFor(() => expect(result.current.state.entries?.totalMatches).toBe(5))
    act(() => result.current.setQuery('foo'))
    act(() => result.current.setQuery('foobar'))
    await waitFor(() => expect(result.current.state.entries?.totalMatches).toBe(2))
    await act(async () => { slow.resolve(page(999)) })
    expect(result.current.state.entries?.totalMatches).toBe(2)
  })

  it('reports cancellation without applying results', async () => {
    hoisted.api.loadDemo.mockResolvedValue(info('D'))
    hoisted.api.scanLog.mockRejectedValue({ code: 'cancelled', message: 'cancelled' })
    const { result } = renderHook(() => useLogSession())
    await act(async () => { await result.current.openDemo() })
    expect(result.current.state.phase).toBe('cancelled')
    expect(result.current.state.summary).toBeNull()
  })

  it('keeps the current session when the file picker is dismissed', async () => {
    hoisted.api.loadDemo.mockResolvedValue(info('D'))
    hoisted.api.scanLog.mockResolvedValue(summary('D', 3))
    hoisted.api.selectLog.mockResolvedValue(null)
    const { result } = renderHook(() => useLogSession())
    await act(async () => { await result.current.openDemo() })
    await act(async () => { await result.current.openFile() })
    expect(result.current.state.session?.sessionId).toBe('D')
    expect(result.current.state.phase).toBe('ready')
  })

  it('resets to page 0 when search or severity changes', async () => {
    hoisted.api.loadDemo.mockResolvedValue(info('D'))
    hoisted.api.scanLog.mockResolvedValue(summary('D', 500))
    const { result } = renderHook(() => useLogSession())
    await act(async () => { await result.current.openDemo() })
    act(() => result.current.setPage(3))
    act(() => result.current.setSeverity('ERROR'))
    expect(result.current.state.page).toBe(0)
  })
})
