import type { ChatEntry, ChatTurn } from '$lib/types/backend'

export type UserEntry = Extract<ChatEntry, { kind: 'user' }>
export type MessageEntry = Extract<ChatEntry, { kind: 'message' }>
export type ThoughtEntry = Extract<ChatEntry, { kind: 'thought' }>
export type ToolEntry = Extract<ChatEntry, { kind: 'tool' }>
export type PlanEntry = Extract<ChatEntry, { kind: 'plan' }>
export type TurnEndEntry = Extract<ChatEntry, { kind: 'turn_end' }>

export type Turn = {
  id: string
  user: UserEntry | null
  steps: ChatEntry[]
  plans: PlanEntry[]
  answer: MessageEntry | null
  end: TurnEndEntry | null
  running: boolean
}

export function transcriptTurns(entries: ChatEntry[], state: ChatTurn): Turn[] {
  const turns: Turn[] = []
  let current: Turn | undefined
  for (const entry of entries) {
    if (entry.kind === 'user' || !current) {
      current = {
        id: entry.kind === 'user' ? entry.id : 'turn:0',
        user: entry.kind === 'user' ? entry : null,
        steps: [],
        plans: [],
        answer: null,
        end: null,
        running: false
      }
      turns.push(current)
    }
    if (entry.kind === 'plan') current.plans.push(entry)
    else if (entry.kind === 'turn_end') current.end = entry
    else if (entry.kind !== 'user') current.steps.push(entry)
  }
  for (const turn of turns) {
    const last = turn.steps.at(-1)
    if (last?.kind === 'message') {
      turn.answer = last
      turn.steps.pop()
    }
  }
  const last = turns.at(-1)
  if (last) last.running = state === 'running'
  return turns
}

export function turnFoldOpen(turn: Turn): boolean {
  return turn.running || !turn.answer || (turn.end !== null && turn.end.stop_reason !== 'end_turn')
}

export function turnSummary(turn: Turn): string {
  let tools = 0
  let messages = 0
  let thoughts = 0
  for (const step of turn.steps) {
    if (step.kind === 'tool') tools++
    else if (step.kind === 'message') messages++
    else if (step.kind === 'thought') thoughts++
  }
  return [
    tools ? `${tools} tool call${tools === 1 ? '' : 's'}` : '',
    messages ? `${messages} message${messages === 1 ? '' : 's'}` : '',
    thoughts ? `${thoughts} thought${thoughts === 1 ? '' : 's'}` : ''
  ]
    .filter(Boolean)
    .join(' · ')
}
