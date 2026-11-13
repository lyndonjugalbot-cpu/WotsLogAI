import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { isTauri, webBackend } from '@/lib/webDemo'
import type {
  AiSettingsView, AnalysisPreview, AnalysisResult, AppErr, DemoAnalysis, PageResult, ScanResult, SessionInfo,
} from '@/types'

export function toAppErr(e: unknown): AppErr {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) return e as AppErr
  return { code: 'unknown', message: typeof e === 'string' ? e : e instanceof Error ? e.message : 'Unknown error' }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw toAppErr(e)
  }
}

export interface QueryArgs {
  sessionId: string
  query: string
  severity: string | null
  offset: number
  limit: number
  scanId: string
}
export interface SaveSettingsArgs {
  baseUrl: string
  model: string
  apiKey: string | null
  clearKey: boolean
}

// Only typed wrappers over the allow-listed commands. No paths and no keys are ever read back from Rust.
const desktopApi = {
  selectLog: () => call<SessionInfo | null>('select_log'),
  loadDemo: () => call<SessionInfo>('load_demo'),
  scanLog: (sessionId: string, scanId: string) => call<ScanResult>('scan_log', { sessionId, scanId }),
  cancelScan: (scanId: string) => call<void>('cancel_scan', { scanId }),
  queryLogs: (a: QueryArgs) => call<PageResult>('query_logs', { ...a }),
  prepareAnalysis: (sessionId: string, groupId: string) =>
    call<AnalysisPreview>('prepare_analysis', { sessionId, groupId }),
  analyzeError: (approvalId: string) => call<AnalysisResult>('analyze_error', { approvalId }),
  demoAnalysis: (sessionId: string, groupId: string) => call<DemoAnalysis>('demo_analysis', { sessionId, groupId }),
  getAiSettings: () => call<AiSettingsView>('get_ai_settings'),
  saveAiSettings: (a: SaveSettingsArgs) => call<AiSettingsView>('save_ai_settings', { ...a }),
  onProgress: (cb: (p: { scanId: string; bytesRead: number; totalBytes: number }) => void) =>
    listen<{ scanId: string; bytesRead: number; totalBytes: number }>('scan-progress', (e) => cb(e.payload)),
}

export const api: typeof desktopApi = isTauri()
  ? desktopApi
  : {
      ...desktopApi,
      ...webBackend,
      cancelScan: async () => {},
      analyzeError: async () => { throw { code: 'aiDisabled', message: 'Live AI is not available in the browser preview.' } },
      onProgress: async () => () => {},
    }
