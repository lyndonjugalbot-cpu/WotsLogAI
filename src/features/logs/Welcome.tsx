import { FileText, FlaskConical, ShieldCheck } from 'lucide-react'
import { Button } from '@/components/ui/button'

export function Welcome({ onOpen, onDemo, error }: { onOpen: () => void; onDemo: () => void; error?: string | null }) {
  return (
    <main className="mx-auto flex h-full max-w-2xl flex-col justify-center gap-6 px-6">
      <div>
        <h1 className="text-3xl font-semibold tracking-tight">WotsLog AI</h1>
        <p className="mt-2 text-muted-foreground">
          Turn noisy application logs into debugging insights. Search, group repeated errors, and ask AI to explain them,
          while your raw log files stay on this device.
        </p>
      </div>
      {error && (
        <p role="alert" className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-destructive">
          {error}
        </p>
      )}
      <div className="flex flex-wrap gap-3">
        <Button onClick={onOpen}>
          <FileText className="size-4" /> Open log file
        </Button>
        <Button variant="outline" onClick={onDemo}>
          <FlaskConical className="size-4" /> Try the demo log
        </Button>
      </div>
      <p className="text-xs text-muted-foreground">Supports .log, .txt and newline-delimited JSON, up to 100 MB.</p>
      <section aria-label="Privacy" className="rounded-lg border bg-card p-4">
        <h2 className="flex items-center gap-2 font-medium">
          <ShieldCheck className="size-4 text-ok" /> Privacy by default
        </h2>
        <ul className="mt-2 list-disc space-y-1 pl-5 text-muted-foreground">
          <li>All parsing and search happen locally. No telemetry, no uploads.</li>
          <li>Nothing leaves your machine unless you click Analyze, review the exact redacted excerpt, and approve it.</li>
          <li>API keys are kept in the operating system credential store and never shown to the interface.</li>
          <li>The demo log is synthetic and works fully offline, with a clearly labeled example analysis.</li>
        </ul>
      </section>
    </main>
  )
}
