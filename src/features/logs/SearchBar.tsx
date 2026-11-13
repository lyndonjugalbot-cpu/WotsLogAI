import { Search, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

/** Debounced search box; the committed value (from the hook) is the source of truth. */
export function SearchBar({ value, onCommit }: { value: string; onCommit: (v: string) => void }) {
  const [text, setText] = useState(value)
  const last = useRef(value)
  useEffect(() => {
    if (value !== last.current) {
      last.current = value
      setText(value)
    }
  }, [value])
  useEffect(() => {
    if (text === last.current) return
    const t = setTimeout(() => {
      last.current = text
      onCommit(text)
    }, 250)
    return () => clearTimeout(t)
  }, [text, onCommit])
  return (
    <div className="relative flex-1">
      <Search className="pointer-events-none absolute top-2.5 left-2.5 size-4 text-muted-foreground" />
      <Input
        aria-label="Search log lines"
        placeholder="Search text (case-insensitive, literal)"
        className="pr-8 pl-8"
        value={text}
        maxLength={512}
        onChange={(e) => setText(e.target.value)}
      />
      {text && (
        <Button variant="ghost" size="icon" className="absolute top-0.5 right-0.5 size-8" aria-label="Clear search" onClick={() => setText('')}>
          <X className="size-4" />
        </Button>
      )}
    </div>
  )
}
