import type { ChatBlock, ChatContent, ChatEntry, ChatEntryBody, ChatLive } from '$lib/types/backend'

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
        | Extract<ChatEntryBody, { kind: 'notice' | 'modelChange' | 'thinkingChange' | 'compaction' | 'branchSummary' }>
      ))
export type Turn = {
  id: string
  user: UserEntry | null
  steps: Step[]
  assistants: AssistantEntry[]
  answer: AssistantEntry | null
  end: Extract<ChatEntryBody, { kind: 'runEnd' }> | null
  running: boolean
}

export function transcriptTurns(entries: ChatEntry[], streaming: ChatBlock[] | null, live: ChatLive): Turn[] {
  const turns: Turn[] = []
  const results = new Map<string, ToolResult>()
  for (const entry of entries) {
    if (entry.body.kind === 'toolResult') results.set(entry.body.callId, entry.body)
  }
  const runs = new Map(live.tools.map((run) => [run.callId, run]))
  let current: Turn | undefined
  function start(user: UserEntry | null): Turn {
    const turn: Turn = {
      id: user?.id ?? 'turn:0',
      user,
      steps: [],
      assistants: [],
      answer: null,
      end: null,
      running: false
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
    if (!current && (body.kind === 'modelChange' || body.kind === 'thinkingChange')) continue
    if (body.kind === 'toolResult') continue
    current ??= start(null)
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
  for (const turn of turns) {
    const last = turn.assistants.at(-1)
    if (
      !turn.running &&
      last?.body.blocks.some((block) => block.type === 'text') &&
      !last.body.blocks.some((block) => block.type === 'toolCall')
    ) {
      turn.answer = last
      turn.steps = turn.steps.filter((step) => step.entryId !== last.id || step.kind !== 'text')
    }
  }
  return turns
}

export function turnFoldOpen(turn: Turn): boolean {
  return turn.running || !turn.answer || (turn.end !== null && turn.end.outcome !== 'done')
}

export function turnSummary(turn: Turn): string {
  return (
    [
      ['tool', 'tool call'],
      ['text', 'message'],
      ['thinking', 'thought']
    ] as const
  )
    .map(([kind, label]) => {
      const n = turn.steps.filter((step) => step.kind === kind).length
      return n ? `${n} ${label}${n === 1 ? '' : 's'}` : ''
    })
    .filter(Boolean)
    .join(' · ')
}
