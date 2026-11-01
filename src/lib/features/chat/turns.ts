import type { ChatBlock, ChatContent, ChatEntry, ChatEntryBody, ChatLive, ChatThinking } from '$lib/types/backend'

export type UserEntry = ChatEntry & { body: Extract<ChatEntryBody, { kind: 'user' }> }
export type AssistantEntry = ChatEntry & { body: Extract<ChatEntryBody, { kind: 'assistant' }> }
export type ToolResult = Extract<ChatEntryBody, { kind: 'toolResult' }>
type StepBase = { id: string; entryId: string | null }
export type ToolStep = StepBase & {
  kind: 'tool'
  callId: string
  name: string
  arguments: unknown
  status: 'running' | 'done' | 'error'
  result: ToolResult | null
  output: ChatContent[]
}
export type Step =
  | ToolStep
  | (StepBase &
      (
        | { kind: 'text'; text: string }
        | { kind: 'thinking'; text: string; redacted: boolean }
        | Extract<ChatEntryBody, { kind: 'notice' | 'compaction' | 'branchSummary' }>
      ))
/** Model and thinking that changed since the previous user message; null where unchanged. */
export type TurnConfig = { model: string | null; thinking: ChatThinking | null }
export type Turn = {
  id: string
  user: UserEntry | null
  config: TurnConfig
  steps: Step[]
  assistants: AssistantEntry[]
  end: Extract<ChatEntryBody, { kind: 'runEnd' }> | null
  running: boolean
  startedAt: string | null
  endedAt: string | null
}

export function transcriptTurns(entries: ChatEntry[], streaming: ChatBlock[] | null, live: ChatLive): Turn[] {
  const turns: Turn[] = []
  const results = new Map<string, ToolResult>()
  for (const entry of entries) {
    if (entry.body.kind === 'toolResult') results.set(entry.body.callId, entry.body)
  }
  const runs = new Map(live.tools.map((run) => [run.callId, run]))
  let current: Turn | undefined
  // axo logs every picker step; only the net settings a message was sent with matter.
  const settings: TurnConfig = { model: null, thinking: null }
  let sent: TurnConfig | null = null
  function start(user: UserEntry | null, timestamp: string | null = user?.timestamp ?? null): Turn {
    const config: TurnConfig = { model: null, thinking: null }
    if (user) {
      if (sent) {
        if (settings.model !== sent.model) config.model = settings.model
        if (settings.thinking !== sent.thinking) config.thinking = settings.thinking
      }
      sent = { ...settings }
    }
    const turn: Turn = {
      id: user?.id ?? 'turn:0',
      user,
      config,
      steps: [],
      assistants: [],
      end: null,
      running: false,
      startedAt: timestamp,
      endedAt: null
    }
    turns.push(turn)
    return turn
  }
  function blocks(turn: Turn, content: ChatBlock[], entryId: string | null) {
    content.forEach((block, index) => {
      const base = { id: `${entryId ?? 'streaming'}:${index}`, entryId }
      if (block.type === 'toolCall') {
        const result = results.get(block.id) ?? null
        const run = runs.get(block.id)
        turn.steps.push({
          ...base,
          kind: 'tool',
          callId: block.id,
          name: block.name,
          arguments: block.arguments,
          status: result ? (result.isError ? 'error' : 'done') : 'running',
          result,
          output: result?.content ?? run?.output ?? []
        })
      } else if (block.type === 'text') {
        turn.steps.push({ ...base, kind: 'text', text: block.text })
      } else {
        turn.steps.push({ ...base, kind: 'thinking', text: block.text, redacted: block.redacted })
      }
    })
  }
  for (const entry of entries) {
    const body = entry.body
    if (body.kind === 'user') {
      current = start(entry as UserEntry)
      continue
    }
    if (body.kind === 'modelChange') {
      settings.model = body.model
      continue
    }
    if (body.kind === 'thinkingChange') {
      settings.thinking = body.thinking
      continue
    }
    current ??= start(null, entry.timestamp)
    current.endedAt = entry.timestamp
    if (body.kind === 'toolResult') continue
    if (body.kind === 'assistant') {
      current.assistants.push(entry as AssistantEntry)
      blocks(current, body.blocks, entry.id)
    } else if (body.kind === 'runEnd') current.end = body
    else current.steps.push({ ...body, id: entry.id, entryId: entry.id })
  }
  if (streaming !== null) {
    current ??= start(null)
    blocks(current, streaming, null)
  }
  if (current) current.running = live.running
  return turns
}

export function turnDurationMs(turn: Turn, now: number): number | null {
  if (turn.startedAt === null || (!turn.running && turn.endedAt === null)) return null
  const start = Date.parse(turn.startedAt)
  const end = turn.running ? now : Date.parse(turn.endedAt!)
  return Number.isFinite(start) && Number.isFinite(end) ? Math.max(0, end - start) : null
}
