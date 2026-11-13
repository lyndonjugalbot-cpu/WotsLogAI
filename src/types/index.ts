export type Severity = 'ERROR' | 'WARN' | 'INFO' | 'DEBUG' | 'UNKNOWN'
export const SEVERITIES: Severity[] = ['ERROR', 'WARN', 'INFO', 'DEBUG', 'UNKNOWN']

export interface LogEntry {
  lineNumber: number
  timestamp?: string | null
  severity: Severity
  message: string
  truncated: boolean
}
export interface ErrorGroup {
  id: string
  signature: string
  count: number
  sampleLineNumbers: number[]
}
export interface Summary {
  totalLines: number
  severityCounts: Record<Severity, number>
  topErrors: ErrorGroup[]
  elapsedMs: number
  groupingTruncated: boolean
  distinctGroups: number
  bytes: number
  truncatedLines: number
  invalidUtf8Lines: number
}
export interface ScanResult extends Summary {
  sessionId: string
  scanId: string
  maxFileBytes: number
}
export interface PageResult {
  sessionId: string
  scanId: string
  entries: LogEntry[]
  totalMatches: number
  offset: number
  limit: number
}
export interface SessionInfo {
  sessionId: string
  displayName: string
  sizeBytes: number
  isDemo: boolean
}
export interface Evidence {
  lineNumber: number
  explanation: string
}
export interface Diagnosis {
  summary: string
  likelyCauses: string[]
  evidence: Evidence[]
  nextSteps: string[]
  uncertainty: string
}
export interface ProviderView {
  baseUrl: string
  model: string
  host: string
}
export interface AnalysisPreview {
  approvalId: string
  groupId: string
  signature: string
  lineNumbers: number[]
  outboundMessage: string
  systemPrompt: string
  provider: ProviderView
  sendsApiKey: boolean
  redactions: Record<string, number>
  outboundBytes: number
  expiresInSecs: number
}
export interface AnalysisResult {
  diagnosis: Diagnosis
  isDemo: boolean
  provider: ProviderView
}
export interface DemoAnalysis {
  isDemo: true
  label: string
  diagnosis: Diagnosis
}
export interface AiSettingsView {
  baseUrl: string
  model: string
  hasKey: boolean
  configured: boolean
  credentialStoreOk: boolean
  liveAiAvailable: boolean
  liveAiReason?: string | null
}
export interface AppErr {
  code: string
  message: string
  retryAllowed?: boolean
}
