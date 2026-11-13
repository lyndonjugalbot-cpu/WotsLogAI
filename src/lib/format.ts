import type { Diagnosis } from '@/types'

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 ** 2) return `${(n / 1024).toFixed(1)} KiB`
  if (n < 1024 ** 3) return `${(n / 1024 ** 2).toFixed(1)} MiB`
  return `${(n / 1024 ** 3).toFixed(2)} GiB`
}

export function formatMs(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(2)} s`
}

/** Query that finds the raw lines of a group: signature text up to the first normalised placeholder. */
export function groupFilterQuery(signature: string): string {
  const cut = signature.indexOf('<')
  const prefix = (cut > 0 ? signature.slice(0, cut) : signature).trim()
  return prefix.slice(0, 80)
}

export function diagnosisToMarkdown(
  d: Diagnosis,
  opts: { signature: string; demoLabel?: string; provider?: string },
): string {
  const list = (xs: string[]) => (xs.length ? xs.map((x) => `- ${x}`).join('\n') : '- (none)')
  const lines = [
    `## Log analysis: ${opts.signature}`,
    '',
    opts.demoLabel ? `> **${opts.demoLabel}**` : `> AI-generated${opts.provider ? ` via ${opts.provider}` : ''}. Advisory only; not a verified fix.`,
    '',
    '### Summary',
    d.summary,
    '',
    '### Likely causes',
    list(d.likelyCauses),
    '',
    '### Evidence',
    d.evidence.length ? d.evidence.map((e) => `- Line ${e.lineNumber}: ${e.explanation}`).join('\n') : '- (none)',
    '',
    '### Next steps (run nothing automatically)',
    list(d.nextSteps),
    '',
    '### Uncertainty',
    d.uncertainty,
    '',
  ]
  return lines.join('\n')
}

const FRIENDLY: Record<string, string> = {
  fileChanged: 'The file changed on disk. Re-import it to continue.',
  fileTooLarge: 'This file is larger than the 100 MB MVP limit.',
  aiOffline: 'Could not reach the AI provider. Check your connection or provider URL.',
  aiTimeout: 'The AI provider timed out.',
  aiUnauthorized: 'The AI provider rejected the credentials. Check the API key in Settings.',
}
export function friendlyError(code: string, message: string): string {
  return FRIENDLY[code] ?? message
}
