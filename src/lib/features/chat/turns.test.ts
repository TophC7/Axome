import test from 'node:test'
import assert from 'node:assert/strict'
import type { ChatEntry, ChatEntryBody, ChatLive } from '$lib/types/backend'
import { transcriptTurns, turnDurationMs } from './turns'

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

test('steps retain final text in order and pair tool results', () => {
  const [turn] = transcriptTurns(
    [user, work, result, message, entry('compact', { kind: 'compaction', summary: 'Summary', tokensBefore: 10 }), end],
    null,
    live
  )
  assert.equal(turn.user, user)
  assert.deepEqual(
    turn.steps.map((step) => step.kind),
    ['thinking', 'text', 'tool', 'text', 'compaction']
  )
  const call = turn.steps[2]
  assert.equal(call.kind === 'tool' && call.result, result.body)
  assert.equal(turn.end, end.body)
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
  assert.equal(turns[1].running, true)
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
})

test('failed tool results stay paired with their calls', () => {
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
  assert.deepEqual(
    turn.steps.map((step) => step.kind),
    ['thinking', 'text', 'tool']
  )
  const call = turn.steps[2]
  assert.equal(call.kind === 'tool' && call.status, 'error')
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
  assert.deepEqual(
    turns[0].steps.map((step) => step.kind),
    ['text']
  )
  assert.equal(turns[0].startedAt, message.timestamp)
  assert.equal(turns[1].id, user.id)
})

test('picker steps fold into the net change carried by the next user message', () => {
  const thinking = (id: string, level: 'low' | 'medium' | 'high') =>
    entry(id, { kind: 'thinkingChange', thinking: level })
  const turns = transcriptTurns(
    [
      thinking('t0', 'low'),
      user,
      message,
      thinking('t1', 'medium'),
      thinking('t2', 'high'),
      { ...user, id: 'u2' },
      thinking('t3', 'low'),
      thinking('t4', 'high'),
      { ...user, id: 'u3' }
    ],
    null,
    live
  )
  assert.deepEqual(
    turns.map((turn) => turn.config),
    [
      { model: null, thinking: null },
      { model: null, thinking: 'high' },
      { model: null, thinking: null }
    ]
  )
  assert.ok(turns.every((turn) => turn.steps.every((step) => step.kind === 'text')))
})

test('replayed turns retain text without an invented run end', () => {
  const [turn] = transcriptTurns([user, message], null, live)
  assert.equal(turn.end, null)
  assert.deepEqual(
    turn.steps.map((step) => step.kind),
    ['text']
  )
  assert.deepEqual(transcriptTurns([], null, live), [])
})

test('duration uses entry timestamps when finished and the clock while running', () => {
  const startedAt = '2026-01-01T00:00:00Z'
  const endedAt = '2026-01-01T00:00:12Z'
  const [turn] = transcriptTurns(
    [
      { ...user, timestamp: startedAt },
      { ...message, timestamp: endedAt }
    ],
    null,
    live
  )
  assert.equal(turn.startedAt, startedAt)
  assert.equal(turn.endedAt, endedAt)
  assert.equal(turnDurationMs(turn, Date.parse(endedAt) + 5000), 12_000)
  assert.equal(turnDurationMs({ ...turn, running: true }, Date.parse(endedAt) + 5000), 17_000)
  assert.equal(turnDurationMs({ ...turn, startedAt: null }, Date.now()), null)
  assert.equal(turnDurationMs({ ...turn, endedAt: null }, Date.now()), null)
  const [userOnly] = transcriptTurns([user], null, live)
  assert.equal(userOnly.endedAt, null)
})
