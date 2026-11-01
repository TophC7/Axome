import test from 'node:test'
import assert from 'node:assert/strict'
import type { ToolStep } from '../../turns'
import { searchOutput, type SearchOutputModel } from './searchOutput'

function call(name: 'grep' | 'find', source: string, details: unknown = null): ToolStep {
  const output = [{ type: 'text' as const, text: source }]
  return {
    id: 'tool',
    entryId: 'entry',
    kind: 'tool',
    callId: 'call',
    name,
    arguments: { pattern: '(a+)+$', limit: 2 },
    status: 'done',
    output,
    result: {
      kind: 'toolResult',
      callId: 'call',
      toolName: name,
      content: output,
      details,
      isError: false,
      durationMs: null
    }
  }
}
function lines(output: SearchOutputModel) {
  return output.parts.flatMap((part) => (part.type === 'text' ? part.lines : []))
}
function visible(output: SearchOutputModel): string {
  return output.parts
    .map((part) => (part.type === 'text' ? part.lines.map((row) => row.text).join('') + part.suffix : ''))
    .join('')
}

test('grep hyperlinks exact prefixes without changing source order, context, whitespace, or repeated paths', () => {
  const source = 'z file.ts:9: \tfirst  \na-b:notes.ts-4-  context\nz file.ts:9: repeated\na-b:notes.ts:5: \n'
  const output = searchOutput(call('grep', source))
  assert.equal(visible(output), source)
  assert.deepEqual(
    lines(output).map((row) => row.link),
    [
      { path: 'z file.ts', line: 9, length: 'z file.ts:9'.length },
      { path: 'a-b:notes.ts', line: 4, length: 'a-b:notes.ts-4'.length },
      { path: 'z file.ts', line: 9, length: 'z file.ts:9'.length },
      { path: 'a-b:notes.ts', line: 5, length: 'a-b:notes.ts:5'.length }
    ]
  )
})

test('ambiguous delimiters and unknown rows remain raw without hiding valid neighboring results', () => {
  const raw = [
    'file:4: value:8: other',
    'file-4- value:8: other',
    'odd:4: name.ts:8: source',
    'unknown output',
    'file:0: hit',
    'file:9007199254740992: hit',
    '/absolute.ts:1: hit',
    '../escape.ts:1: hit',
    'a\r.ts:1: hit'
  ]
  const source = ['good.ts:1: hit', ...raw, 'good.ts:2: hit'].join('\n')
  const output = searchOutput(call('grep', source))
  assert.equal(visible(output), source)
  assert.equal(lines(output)[0].link?.path, 'good.ts')
  assert.ok(
    lines(output)
      .slice(1, -1)
      .every((row) => !row.link)
  )
  assert.equal(lines(output).at(-1)?.link?.line, 2)
})

test('find preserves exact paths and only recognizes notices proven by metadata and paragraph position', () => {
  const filename = '[50.0KB limit reached]'
  const source = ` nested folder /\nnested/ a.ts \n${filename}`
  const output = searchOutput(call('find', source))
  assert.equal(visible(output), source)
  assert.deepEqual(
    lines(output).map((row) => row.link?.path),
    [' nested folder /', 'nested/ a.ts ', filename]
  )
  const notice = '[2 results limit reached. Use limit=4 for more, or refine pattern]'
  const withNotice = `${source}\n\n${notice}`
  const verified = searchOutput(call('find', withNotice, { resultLimitReached: 2 }))
  assert.equal(visible(verified), withNotice)
  assert.deepEqual(
    lines(verified).map((row) => row.link?.path),
    [' nested folder /', 'nested/ a.ts ', filename]
  )
  for (const mismatch of [withNotice.replace('limit=4', 'limit=5'), `${withNotice}\nextra`]) {
    const fallback = searchOutput(call('find', mismatch, { resultLimitReached: 2 }))
    assert.equal(visible(fallback), mismatch)
    assert.ok(lines(fallback).every((row) => !row.link))
  }
  const error = call('find', source)
  error.status = 'error'
  error.result!.isError = true
  assert.ok(lines(searchOutput(error)).every((row) => !row.link))
})

test('preview keeps the first twelve logical lines and verified trailing notices; copy keeps the entire recording', () => {
  const body = Array.from({ length: 15 }, (_, index) => `file.ts:${index + 1}: ${index}`).join('\n')
  const notice = '[Some lines truncated to 500 chars. Use read tool to see full lines]'
  const source = `${body}\n\n${notice}`
  const step = call('grep', source, { linesTruncated: true })
  const preview = searchOutput(step, 12)
  assert.equal(preview.hidden, 3)
  assert.equal(lines(preview).length, 12)
  assert.equal(lines(preview)[0].link?.line, 1)
  assert.equal(lines(preview).at(-1)?.link?.line, 12)
  assert.equal(visible(preview), `${body.split('\n').slice(0, 12).join('\n')}\n\n${notice}`)
  assert.equal(visible(searchOutput(step)), source)
  assert.equal(searchOutput(step).hidden, 0)
})
