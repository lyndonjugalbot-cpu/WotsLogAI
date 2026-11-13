import { ClipboardCopy, FlaskConical, Loader2, Send, ShieldAlert, X } from 'lucide-react'
import { useCallback, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { diagnosisToMarkdown, friendlyError } from '@/lib/format'
import { api, toAppErr } from '@/lib/tauri'
import type { AiSettingsView, AnalysisPreview, AppErr, Diagnosis, ErrorGroup, SessionInfo } from '@/types'

type Step =
  | { kind: 'loading' }
  | { kind: 'unavailable'; reason: string }
  | { kind: 'preview'; preview: AnalysisPreview }
  | { kind: 'sending'; preview: AnalysisPreview }
  | { kind: 'failed'; preview: AnalysisPreview | null; error: AppErr }
  | { kind: 'done'; diagnosis: Diagnosis; demoLabel?: string; provider?: string }

export function AnalysisPanel({
  session, group, settings, onClose, onOpenSettings,
}: {
  session: SessionInfo
  group: ErrorGroup
  settings: AiSettingsView | null
  onClose: () => void
  onOpenSettings: () => void
}) {
  const [step, setStep] = useState<Step>({ kind: 'loading' })
  const [consent, setConsent] = useState(false)
  const [copied, setCopied] = useState(false)

  const prepare = useCallback(async () => {
    setConsent(false)
    setStep({ kind: 'loading' })
    try {
      setStep({ kind: 'preview', preview: await api.prepareAnalysis(session.sessionId, group.id) })
    } catch (e) {
      const err = toAppErr(e)
      setStep(err.code === 'aiDisabled' || err.code === 'aiNotConfigured'
        ? { kind: 'unavailable', reason: settings?.liveAiReason || err.message }
        : { kind: 'failed', preview: null, error: err })
    }
  }, [session.sessionId, group.id, settings?.liveAiReason])

  useEffect(() => {
    void prepare()
  }, [prepare])

  async function send(preview: AnalysisPreview) {
    setStep({ kind: 'sending', preview })
    try {
      const r = await api.analyzeError(preview.approvalId)
      setStep({ kind: 'done', diagnosis: r.diagnosis, provider: `${r.provider.model} @ ${r.provider.host}` })
    } catch (e) {
      setStep({ kind: 'failed', preview, error: toAppErr(e) })
    }
  }

  async function showDemo() {
    try {
      const r = await api.demoAnalysis(session.sessionId, group.id)
      setStep({ kind: 'done', diagnosis: r.diagnosis, demoLabel: r.label })
    } catch (e) {
      setStep({ kind: 'failed', preview: null, error: toAppErr(e) })
    }
  }

  async function copy(d: Diagnosis, demoLabel?: string, provider?: string) {
    try {
      await navigator.clipboard.writeText(diagnosisToMarkdown(d, { signature: group.signature, demoLabel, provider }))
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    } catch {
      setStep({ kind: 'failed', preview: null, error: { code: 'clipboard', message: 'Could not access the clipboard.' } })
    }
  }

  return (
    <aside aria-label="AI analysis" className="flex h-full min-h-0 flex-col gap-3 overflow-auto rounded-lg border bg-card p-4">
      <div className="flex items-start justify-between gap-2">
        <div>
          <h2 className="font-semibold">AI analysis</h2>
          <p className="mt-1 font-mono text-xs break-words text-muted-foreground" title={group.signature}>
            {group.signature.slice(0, 160)} (×{group.count.toLocaleString()})
          </p>
        </div>
        <Button variant="ghost" size="icon" aria-label="Close analysis" onClick={onClose}><X className="size-4" /></Button>
      </div>

      {step.kind === 'loading' && <p className="flex items-center gap-2 text-muted-foreground"><Loader2 className="size-4 animate-spin" /> Preparing a redacted excerpt…</p>}

      {step.kind === 'unavailable' && (
        <div className="space-y-3">
          <p role="status" className="rounded-md border border-warn/50 bg-warn/10 p-3 text-warn">
            <ShieldAlert className="mr-1 inline size-4" /> Live AI is off: {step.reason} Nothing was sent anywhere.
          </p>
          {session.isDemo ? (
            <Button onClick={showDemo}><FlaskConical className="size-4" /> Show demo analysis (offline example)</Button>
          ) : (
            <p className="text-muted-foreground">Example analyses exist only for the bundled demo log.</p>
          )}
          <Button variant="outline" onClick={onOpenSettings}>Configure AI provider</Button>
        </div>
      )}

      {(step.kind === 'preview' || step.kind === 'sending' || (step.kind === 'failed' && step.preview)) && (() => {
        const preview = step.kind === 'failed' ? step.preview! : step.preview
        const busy = step.kind === 'sending'
        return (
          <div className="space-y-3">
            <p className="rounded-md border border-primary/40 bg-primary/10 p-3">
              Review exactly what will leave this device. Destination:{' '}
              <strong>{preview.provider.host}</strong> (model <strong>{preview.provider.model}</strong>).{' '}
              {preview.sendsApiKey ? 'Your stored API key is sent as the Authorization header (never shown here).' : 'No API key is configured, so none is sent.'}
            </p>
            <div>
              <h3 className="mb-1 text-xs font-semibold tracking-wide text-muted-foreground">
                OUTBOUND MESSAGE: {preview.lineNumbers.length} lines, {preview.outboundBytes.toLocaleString()} bytes
              </h3>
              <pre aria-label="Exact outbound message" className="max-h-72 overflow-auto rounded-md border bg-background p-3 font-mono text-xs whitespace-pre-wrap">{preview.outboundMessage}</pre>
            </div>
            <details className="text-xs text-muted-foreground">
              <summary className="cursor-pointer">System prompt also sent</summary>
              <p className="mt-1 whitespace-pre-wrap">{preview.systemPrompt}</p>
            </details>
            <p className="text-xs text-muted-foreground">
              Redacted: {Object.keys(preview.redactions).length === 0 ? 'nothing matched the redaction rules' : Object.entries(preview.redactions).map(([k, v]) => `${v} ${k}`).join(', ')}.
              Redaction is best-effort, not a guarantee: read the text above before sending.
            </p>
            <label className="flex items-start gap-2">
              <input type="checkbox" className="mt-0.5" checked={consent} onChange={(e) => setConsent(e.target.checked)} disabled={busy} />
              <span>I reviewed this excerpt and approve sending exactly this text to {preview.provider.host}.</span>
            </label>
            <div className="flex gap-2">
              {(step.kind !== 'failed' || step.error.retryAllowed) && (
                <Button disabled={!consent || busy} onClick={() => send(preview)}>
                  {busy ? <Loader2 className="size-4 animate-spin" /> : <Send className="size-4" />}
                  {busy ? 'Waiting for response…' : step.kind === 'failed' ? 'Retry once' : 'Approve and send'}
                </Button>
              )}
              <Button variant="outline" disabled={busy} onClick={onClose}>Cancel, send nothing</Button>
            </div>
            {step.kind === 'failed' && (
              <p role="alert" className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-destructive">
                {friendlyError(step.error.code, step.error.message)}{' '}
                {step.error.retryAllowed ? 'One retry of the same approved request is available.' : 'The approval has been used up; prepare a new preview to try again.'}
              </p>
            )}
            {step.kind === 'failed' && !step.error.retryAllowed && <Button variant="outline" onClick={prepare}>Prepare a new preview</Button>}
          </div>
        )
      })()}

      {step.kind === 'failed' && !step.preview && (
        <p role="alert" className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-destructive">{friendlyError(step.error.code, step.error.message)}</p>
      )}

      {step.kind === 'done' && (
        <div className="space-y-3">
          {step.demoLabel ? (
            <p role="note" className="rounded-md border border-warn/50 bg-warn/10 p-2 text-xs font-semibold text-warn">{step.demoLabel}</p>
          ) : (
            <p role="note" className="rounded-md border p-2 text-xs text-muted-foreground">
              AI-generated via {step.provider}. Advisory only; verify before acting. Suggested commands are never run automatically.
            </p>
          )}
          <DiagnosisView d={step.diagnosis} />
          <Button variant="outline" onClick={() => copy(step.diagnosis, step.demoLabel, step.provider)}>
            <ClipboardCopy className="size-4" /> {copied ? 'Copied' : 'Copy as Markdown'}
          </Button>
        </div>
      )}
    </aside>
  )
}

function DiagnosisView({ d }: { d: Diagnosis }) {
  const List = ({ items }: { items: string[] }) => <ul className="list-disc space-y-1 pl-5">{items.map((x, i) => <li key={i}>{x}</li>)}</ul>
  return (
    <div className="space-y-3 text-sm">
      <section><h3 className="font-semibold">Summary</h3><p>{d.summary}</p></section>
      <section><h3 className="font-semibold">Likely causes</h3><List items={d.likelyCauses} /></section>
      <section>
        <h3 className="font-semibold">Evidence</h3>
        <ul className="list-disc space-y-1 pl-5">{d.evidence.map((e, i) => <li key={i}><span className="font-mono text-xs">line {e.lineNumber}</span>: {e.explanation}</li>)}</ul>
      </section>
      <section><h3 className="font-semibold">Next steps</h3><List items={d.nextSteps} /></section>
      <section><h3 className="font-semibold">Uncertainty</h3><p className="text-muted-foreground">{d.uncertainty}</p></section>
    </div>
  )
}
