import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api, toAppErr } from '@/lib/tauri'
import type { AiSettingsView } from '@/types'

export function SettingsDialog({
  open, settings, onClose, onSaved,
}: { open: boolean; settings: AiSettingsView | null; onClose: () => void; onSaved: (s: AiSettingsView) => void }) {
  const ref = useRef<HTMLDialogElement>(null)
  const [baseUrl, setBaseUrl] = useState('')
  const [model, setModel] = useState('')
  const [key, setKey] = useState('')
  const [err, setErr] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    const d = ref.current
    if (!d) return
    if (open && !d.open) {
      setBaseUrl(settings?.baseUrl ?? '')
      setModel(settings?.model ?? '')
      setKey('')
      setErr(null)
      d.showModal()
    } else if (!open && d.open) d.close()
  }, [open, settings])

  async function save(clearKey: boolean) {
    setBusy(true)
    setErr(null)
    try {
      const s = await api.saveAiSettings({ baseUrl, model, apiKey: key || null, clearKey })
      setKey('') // the key is never kept in the UI after saving
      onSaved(s)
      onClose()
    } catch (e) {
      setErr(toAppErr(e).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <dialog ref={ref} onClose={onClose} aria-label="AI provider settings" className="m-auto w-[28rem] max-w-[95vw] rounded-lg border bg-card p-5 text-foreground backdrop:bg-black/60">
      <form method="dialog" onSubmit={(e) => { e.preventDefault(); void save(false) }} className="space-y-3">
        <h2 className="text-lg font-semibold">AI provider</h2>
        <p className="text-xs text-muted-foreground">
          Any OpenAI-compatible chat-completions endpoint. HTTPS is required (plain HTTP only for localhost). Nothing is sent until you approve a specific excerpt.
        </p>
        <label className="block space-y-1"><span>Base URL</span><Input value={baseUrl} onChange={(e) => setBaseUrl(e.target.value)} placeholder="https://api.openai.com/v1" required /></label>
        <label className="block space-y-1"><span>Model</span><Input value={model} onChange={(e) => setModel(e.target.value)} placeholder="gpt-4o-mini" required /></label>
        <label className="block space-y-1">
          <span>API key (optional)</span>
          <Input type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} placeholder={settings?.hasKey ? 'Stored in the OS credential store' : 'Not set'} />
        </label>
        <p className="text-xs text-muted-foreground" aria-live="polite">
          Credential store: {settings?.credentialStoreOk === false ? 'unavailable (live AI disabled)' : settings?.hasKey ? 'key saved (hidden)' : 'no key saved'}.
          {' '}Live AI: {settings?.liveAiAvailable ? 'available' : `off${settings?.liveAiReason ? ` (${settings.liveAiReason})` : ''}`}.
        </p>
        {err && <p role="alert" className="text-destructive">{err}</p>}
        <div className="flex flex-wrap justify-between gap-2 pt-2">
          <Button type="button" variant="outline" disabled={busy || !settings?.hasKey} onClick={() => save(true)}>Clear saved key</Button>
          <div className="flex gap-2">
            <Button type="button" variant="ghost" onClick={onClose}>Close</Button>
            <Button type="submit" disabled={busy}>Save</Button>
          </div>
        </div>
      </form>
    </dialog>
  )
}
