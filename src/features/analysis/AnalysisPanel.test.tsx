import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { beforeEach, vi } from 'vitest'
import type { AnalysisPreview, ErrorGroup, SessionInfo } from '@/types'

const hoisted = vi.hoisted(() => ({ api: {} as Record<string, ReturnType<typeof vi.fn>> }))
vi.mock('@/lib/tauri', () => ({
  api: hoisted.api,
  toAppErr: (e: unknown) => (e && typeof e === 'object' && 'code' in e ? e : { code: 'unknown', message: String(e) }),
}))
import { AnalysisPanel } from './AnalysisPanel'

const group: ErrorGroup = { id: 'g1', signature: 'db refused', count: 3, sampleLineNumbers: [2, 4, 9] }
const live: SessionInfo = { sessionId: 's', displayName: 'a.log', sizeBytes: 1, isDemo: false }
const demo: SessionInfo = { ...live, isDemo: true }
const preview: AnalysisPreview = {
  approvalId: 'ap1', groupId: 'g1', signature: 'db refused', lineNumbers: [2], outboundMessage: 'SCHEMA...\nL2 | - ERROR | refused for [REDACTED_EMAIL]\n',
  systemPrompt: 'sys', provider: { baseUrl: 'https://x/v1', model: 'm1', host: 'x' }, sendsApiKey: true,
  redactions: { email: 1 }, outboundBytes: 50, expiresInSecs: 600,
}
const diag = { summary: '<img src=x onerror=alert(1)> boom', likelyCauses: ['c'], evidence: [{ lineNumber: 2, explanation: 'e' }], nextSteps: ['n'], uncertainty: 'u' }
const noop = () => {}

beforeEach(() => {
  Object.assign(hoisted.api, { prepareAnalysis: vi.fn(), analyzeError: vi.fn(), demoAnalysis: vi.fn() })
})

describe('AnalysisPanel consent and labeling', () => {
  it('shows the exact outbound text and blocks sending until the user approves', async () => {
    hoisted.api.prepareAnalysis.mockResolvedValue(preview)
    hoisted.api.analyzeError.mockResolvedValue({ diagnosis: diag, isDemo: false, provider: preview.provider })
    render(<AnalysisPanel session={live} group={group} settings={null} onClose={noop} onOpenSettings={noop} />)
    const pre = await screen.findByLabelText('Exact outbound message')
    expect(pre.textContent).toBe(preview.outboundMessage)
    const send = screen.getByRole('button', { name: /approve and send/i })
    expect(send).toBeDisabled()
    expect(hoisted.api.analyzeError).not.toHaveBeenCalled()
    await userEvent.click(screen.getByRole('checkbox'))
    await userEvent.click(send)
    await waitFor(() => expect(hoisted.api.analyzeError).toHaveBeenCalledWith('ap1'))
  })

  it('renders AI output as escaped text, never as HTML', async () => {
    hoisted.api.prepareAnalysis.mockResolvedValue(preview)
    hoisted.api.analyzeError.mockResolvedValue({ diagnosis: diag, isDemo: false, provider: preview.provider })
    const { container } = render(<AnalysisPanel session={live} group={group} settings={null} onClose={noop} onOpenSettings={noop} />)
    await userEvent.click(await screen.findByRole('checkbox'))
    await userEvent.click(screen.getByRole('button', { name: /approve and send/i }))
    expect(await screen.findByText(/<img src=x onerror=alert\(1\)> boom/)).toBeInTheDocument()
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByText(/AI-generated via m1 @ x/)).toBeInTheDocument()
  })

  it('offers only a labeled offline example when live AI is unavailable (demo log)', async () => {
    hoisted.api.prepareAnalysis.mockRejectedValue({ code: 'aiDisabled', message: 'Live AI is disabled: not configured' })
    hoisted.api.demoAnalysis.mockResolvedValue({ isDemo: true, label: 'DEMO: static example analysis', diagnosis: { ...diag, summary: 'example' } })
    render(<AnalysisPanel session={demo} group={group} settings={null} onClose={noop} onOpenSettings={noop} />)
    expect(await screen.findByText(/Live AI is off/)).toBeInTheDocument()
    expect(screen.getByText(/Nothing was sent anywhere/)).toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: /show demo analysis/i }))
    expect(await screen.findByText('DEMO: static example analysis')).toBeInTheDocument()
    expect(hoisted.api.analyzeError).not.toHaveBeenCalled()
  })

  it('does not offer demo analysis for a real file when live AI is off', async () => {
    hoisted.api.prepareAnalysis.mockRejectedValue({ code: 'aiNotConfigured', message: 'AI provider is not configured.' })
    render(<AnalysisPanel session={live} group={group} settings={null} onClose={noop} onOpenSettings={noop} />)
    await screen.findByText(/Live AI is off/)
    expect(screen.queryByRole('button', { name: /show demo analysis/i })).toBeNull()
  })

  it('allows exactly the retry the backend grants and then requires a new preview', async () => {
    hoisted.api.prepareAnalysis.mockResolvedValue(preview)
    hoisted.api.analyzeError
      .mockRejectedValueOnce({ code: 'aiInvalidResponse', message: 'bad json', retryAllowed: true })
      .mockRejectedValueOnce({ code: 'aiInvalidResponse', message: 'bad json', retryAllowed: false })
    render(<AnalysisPanel session={live} group={group} settings={null} onClose={noop} onOpenSettings={noop} />)
    await userEvent.click(await screen.findByRole('checkbox'))
    await userEvent.click(screen.getByRole('button', { name: /approve and send/i }))
    const retry = await screen.findByRole('button', { name: /retry once/i })
    await userEvent.click(retry)
    await screen.findByText(/approval has been used up/i)
    expect(screen.queryByRole('button', { name: /retry once/i })).toBeNull()
    expect(screen.getByRole('button', { name: /prepare a new preview/i })).toBeInTheDocument()
  })
})
