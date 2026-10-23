import test from 'node:test'
import assert from 'node:assert/strict'
import type { ChatEntry, ChatEntryBody, ChatLive } from '$lib/types/backend'
import { transcriptTurns, turnFoldOpen, turnSummary } from './turns'

const live: ChatLive = {
  title: null,
  running: false,
  maintenance: null,
  model: null,
  thinking: 'off',
  tools: [],
  queue: [],
  questions: [],
  retry: null,
  stats: { contextTokens: 0, contextWindow: 0, input: 0, output: 0, cacheRead: 0, cacheWrite: 0, cost: 0 }
}
function entry(id: string, body: ChatEntryBody): ChatEntry {
  return { id, timestamp: '2026-01-01T00:00:00Z', body }
}
const user = entry('user:1', { kind: 'user', text: 'Do the work', anchor: null, siblings: ['user:1'] })
const thought = { type: 'thinking', text: 'Thinking', redacted: false } as const
const message = entry('answer', {
  kind: 'assistant',
  blocks: [{ type: 'text', text: 'Done' }],
  model: 'test/model',
  usage: { input: 1, output: 2, cacheRead: 0, cacheWrite: 0, totalTokens: 3, cost: 0.01 },
  stopReason: 'stop',
  error: null,
  durationMs: 10
})
const work = entry('work', {
  kind: 'assistant',
  blocks: [
    thought,
    { type: 'text', text: 'Working' },
    { type: 'toolCall', id: 'call', name: 'read', arguments: { path: 'file' } }
  ],
  model: 'test/model',
  usage: { input: 1, output: 2, cacheRead: 0, cacheWrite: 0, totalTokens: 3, cost: 0.01 },
  stopReason: 'toolUse',
  error: null,
  durationMs: 10
})
const result = entry('result', {
  kind: 'toolResult',
  callId: 'call',
  toolName: 'read',
  content: [{ type: 'text', text: 'File content' }],
  details: null,
  isError: false,
  durationMs: 1
})
const end = entry('end', { kind: 'runEnd', outcome: 'done', reason: null })

test('trailing markers leave the final message outside the fold and pair tool results', () => {
  const [turn] = transcriptTurns(
    [user, work, result, message, entry('compact', { kind: 'compaction', summary: 'Summary', tokensBefore: 10 }), end],
    null,
    live
  )
  assert.equal(turn.user, user)
  assert.equal(turn.answer, message)
  assert.deepEqual(
    turn.steps.map((step) => step.kind),
    ['thinking', 'text', 'tool', 'compaction']
  )
  const call = turn.steps[2]
  assert.equal(call.kind === 'tool' && call.result, result.body)
  assert.equal(turn.end, end.body)
  assert.equal(turnFoldOpen(turn), false)
  assert.equal(turnSummary(turn), '1 tool call · 1 message · 1 thought')
})

test('only the last turn runs and streaming and live output belong to it', () => {
  const turns = transcriptTurns(
    [user, message, end, entry('user:2', user.body), work],
    [{ type: 'text', text: 'Continuing' }],
    {
      ...live,
      running: true,
      tools: [{ callId: 'call', name: 'read', arguments: {}, output: [{ type: 'text', text: 'Partial' }] }]
    }
  )
  assert.equal(turns[0].running, false)
  assert.equal(turnFoldOpen(turns[0]), false)
  assert.equal(turns[1].running, true)
  assert.equal(turns[1].answer, null)
  assert.deepEqual(turns[1].steps.at(-1), {
    id: 'streaming:0',
    kind: 'text',
    text: 'Continuing',
    entryId: null
  })
  const call = turns[1].steps[2]
  assert.equal(call.kind, 'tool')
  if (call.kind !== 'tool') throw new Error('Expected tool step')
  assert.equal(call.status, 'running')
  assert.deepEqual(call.output, [{ type: 'text', text: 'Partial' }])
  assert.equal(turnFoldOpen(turns[1]), true)
})

test('aborted, failed and interrupted turns remain expanded with an answer', () => {
  for (const outcome of ['aborted', 'failed', 'interrupted'] as const) {
    const [turn] = transcriptTurns([user, message, entry('end', { kind: 'runEnd', outcome, reason: null })], null, live)
    assert.equal(turn.answer, message)
    assert.equal(turnFoldOpen(turn), true)
  }
})

test('a tool-call response is not a final answer and failed tool results stay paired', () => {
  const [turn] = transcriptTurns(
    [
      user,
      work,
      entry('error', { ...(result.body as Extract<ChatEntryBody, { kind: 'toolResult' }>), isError: true }),
      end
    ],
    null,
    live
  )
  assert.equal(turn.answer, null)
  assert.deepEqual(
    turn.steps.map((step) => step.kind),
    ['thinking', 'text', 'tool']
  )
  const call = turn.steps[2]
  assert.equal(call.kind === 'tool' && call.status, 'error')
  assert.equal(turnFoldOpen(turn), true)
})

test('leading content has no fabricated user and leading model selections are dropped', () => {
  const turns = transcriptTurns(
    [
      entry('model', { kind: 'modelChange', model: 'test/model' }),
      entry('thinking', { kind: 'thinkingChange', thinking: 'high' }),
      message,
      user,
      work
    ],
    null,
    live
  )
  assert.equal(turns.length, 2)
  assert.equal(turns[0].user, null)
  assert.equal(turns[0].id, 'turn:0')
  assert.deepEqual(turns[0].steps, [])
  assert.equal(turns[0].answer, message)
  assert.equal(turns[1].id, user.id)
})

test('replayed turns collapse without an invented run end', () => {
  const [turn] = transcriptTurns([user, message], null, live)
  assert.equal(turn.end, null)
  assert.equal(turnFoldOpen(turn), false)
  assert.deepEqual(turn.steps, [])
  assert.equal(turnSummary(turn), '')
  assert.deepEqual(transcriptTurns([], null, live), [])
})
