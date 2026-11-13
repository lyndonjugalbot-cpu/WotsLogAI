import { diagnosisToMarkdown, formatBytes, groupFilterQuery } from './format'

const d = {
  summary: 'DB down',
  likelyCauses: ['pool exhausted'],
  evidence: [{ lineNumber: 7, explanation: 'refused' }],
  nextSteps: ['check db'],
  uncertainty: 'no metrics',
}

describe('format', () => {
  it('formats bytes', () => {
    expect(formatBytes(512)).toBe('512 B')
    expect(formatBytes(1536)).toBe('1.5 KiB')
    expect(formatBytes(100 * 1024 ** 2)).toBe('100.0 MiB')
  })

  it('builds a filter query from the literal part of a signature', () => {
    expect(groupFilterQuery('PaymentService: timed out order=<uuid> request_id=<id>')).toBe('PaymentService: timed out order=')
    expect(groupFilterQuery('plain signature')).toBe('plain signature')
    expect(groupFilterQuery('x'.repeat(200)).length).toBe(80)
  })

  it('marks demo output clearly in copied Markdown', () => {
    const md = diagnosisToMarkdown(d, { signature: 'sig', demoLabel: 'DEMO: static example' })
    expect(md).toContain('> **DEMO: static example**')
    expect(md).toContain('- Line 7: refused')
    expect(md).not.toContain('AI-generated')
  })

  it('marks live output as advisory', () => {
    const md = diagnosisToMarkdown(d, { signature: 'sig', provider: 'm @ host' })
    expect(md).toContain('AI-generated via m @ host. Advisory only')
    expect(md).toContain('run nothing automatically')
  })
})
