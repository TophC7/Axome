import test from 'node:test'
import assert from 'node:assert/strict'
import {
  transcriptTurns,
  turnFoldOpen,
  turnSummary,
  type MessageEntry,
  type PlanEntry,
  type ThoughtEntry,
  type ToolEntry,
  type TurnEndEntry,
  type UserEntry
} from './turns'

const user: UserEntry = { kind: 'user', id: 'user:1', content: [{ type: 'text', text: 'Do the work' }] }
const thought: ThoughtEntry = { kind: 'thought', id: 'thought:1', text: 'Thinking' }
const message: MessageEntry = { kind: 'message', id: 'message:1', text: 'Done' }
const plan: PlanEntry = { kind: 'plan', id: 'plan:1', items: [] }
const end: TurnEndEntry = { kind: 'turn_end', id: 'end:1', stop_reason: 'end_turn', usage: null, error: null }
const tool: ToolEntry = {
  kind: 'tool',
  id: 'tool:1',
  name: null,
  title: 'Read file',
  tool_kind: 'read',
  status: 'completed',
  input: null,
  content: [],
  locations: []
}

test('a trailing plan and turn end leave the final message outside the fold', () => {
  const intermediate: MessageEntry = { ...message, id: 'message:0', text: 'Working' }
  const [turn] = transcriptTurns([user, thought, intermediate, tool, message, plan, end], 'idle')
  assert.equal(turn.id, user.id)
  assert.equal(turn.user, user)
  assert.deepEqual(turn.steps, [thought, intermediate, tool])
  assert.deepEqual(turn.plans, [plan])
  assert.equal(turn.answer, message)
  assert.equal(turn.end, end)
  assert.equal(turnFoldOpen(turn), false)
  assert.equal(turnSummary(turn), '1 tool call · 1 message · 1 thought')
})

test('only the last turn runs and it stays expanded even with an answer', () => {
  const turns = transcriptTurns([user, thought, message, end, { ...user, id: 'user:2' }, thought, message], 'running')
  assert.equal(turns[0].running, false)
  assert.equal(turnFoldOpen(turns[0]), false)
  assert.equal(turns[1].running, true)
  assert.equal(turnFoldOpen(turns[1]), true)
})

test('cancelled and error turns stay expanded even with an answer', () => {
  for (const stop_reason of ['cancelled', 'error'] as const) {
    const [turn] = transcriptTurns([user, thought, message, { ...end, stop_reason }], 'idle')
    assert.equal(turnFoldOpen(turn), true)
  }
})

test('a turn with no final answer keeps its intermediate message inside the fold', () => {
  const [turn] = transcriptTurns([user, message, tool, plan, end], 'idle')
  assert.equal(turn.answer, null)
  assert.deepEqual(turn.steps, [message, tool])
  assert.equal(turnFoldOpen(turn), true)
})

test('entries before the first user form a separate turn without a fabricated user', () => {
  const turns = transcriptTurns([thought, message, user, tool], 'idle')
  assert.equal(turns.length, 2)
  assert.equal(turns[0].id, 'turn:0')
  assert.equal(turns[0].user, null)
  assert.deepEqual(turns[0].steps, [thought])
  assert.equal(turns[0].answer, message)
  assert.equal(turns[1].id, user.id)
  assert.equal(turns[1].answer, null)
})

test('replayed turns collapse with an answer without inventing a turn end', () => {
  const [turn] = transcriptTurns([user, thought, message], 'idle')
  assert.equal(turn.end, null)
  assert.equal(turnFoldOpen(turn), false)
  assert.equal(turnSummary(turn), '1 thought')
  const [answerOnly] = transcriptTurns([user, message], 'idle')
  assert.deepEqual(answerOnly.steps, [])
  assert.equal(turnSummary(answerOnly), '')
  assert.deepEqual(transcriptTurns([], 'idle'), [])
})
