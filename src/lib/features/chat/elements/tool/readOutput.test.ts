import test from 'node:test'
import assert from 'node:assert/strict'
import type { ChatContent } from '$lib/types/backend'
import type { ToolStep } from '../../turns'
import { toolHeader } from './toolHeader'
import { readOutput, readStartLine } from './readOutput'

const text = (text: string): ChatContent => ({ type: 'text', text })
const ledger = { path: '/actual/source.ts', access: 'read', sha256: 'recorded' }
function call(args: Record<string, unknown>, output: string | ChatContent[], details: unknown = null): ToolStep {
  const content = typeof output === 'string' ? [text(output)] : output
  return {
    id: 'tool',
    entryId: 'entry',
    kind: 'tool',
    callId: 'call',
    name: 'read',
    arguments: args,
    status: 'done',
    output: content,
    result: { kind: 'toolResult', callId: 'call', toolName: 'read', content, details, isError: false, durationMs: null }
  }
}
function cut(body: string, overrides: Record<string, unknown> = {}) {
  return {
    truncated: true,
    truncatedBy: 'bytes',
    totalLines: 3000,
    totalBytes: 60000,
    outputLines: body === '' ? 0 : body.split('\n').length,
    outputBytes: new TextEncoder().encode(body).length,
    maxLines: 2000,
    maxBytes: 51200,
    lastLinePartial: false,
    firstLineExceedsLimit: false,
    ...overrides
  }
}

test('read exposes exact source separately from its verified footer and attached context', () => {
  const body = '\tfirst  \n second'
  const notice = '[12 more lines in file. Use offset=6 to continue.]'
  const attachment = text('Attached project instructions\nnot source\nnot source')
  const step = call({ path: 'requested.ts', offset: 4.8, limit: 2.9 }, [text(`${body}\n\n${notice}`), attachment], {
    ledger
  })
  const output = step.output
  assert.deepEqual(readOutput(step), {
    path: ledger.path,
    body,
    startLine: 4,
    endLine: 5,
    lineCount: 2,
    notice,
    attachments: [attachment]
  })
  assert.strictEqual(step.output, output)
  assert.equal(step.output[0].type === 'text' && step.output[0].text, `${body}\n\n${notice}`)
  const header = toolHeader(step, '/actual')
  assert.equal(header.path, ledger.path)
  assert.equal(header.line, 4)
  assert.equal(header.pathLabel, 'source.ts:4-5')
})

test('read preserves trailing blank source rows and never invents a range for zero returned lines', () => {
  const trailing = readOutput(call({ path: 'a', offset: -4 }, 'one\ntwo\n'))!
  assert.equal(trailing.body, 'one\ntwo\n')
  assert.equal(trailing.startLine, 1)
  assert.equal(trailing.endLine, 3)
  for (const limit of [0, -2, 0.9]) {
    const source = readOutput(
      call({ path: 'a', limit }, '\n\n[10 more lines in file. Use offset=1 to continue.]', { ledger })
    )!
    assert.equal(source.body, '')
    assert.equal(source.lineCount, 0)
    assert.equal(source.endLine, undefined)
  }
  const blankLine = readOutput(
    call({ path: 'a', limit: 1 }, '\n\n[9 more lines in file. Use offset=2 to continue.]', { ledger })
  )!
  assert.equal(blankLine.body, '')
  assert.equal(blankLine.lineCount, 1)
  assert.equal(blankLine.endLine, 1)
  assert.equal(readOutput(call({ path: 'a' }, ''))!.endLine, undefined)
})

test('truncated ranges require exact recorded source bytes, line counts, and continuation footer', () => {
  for (const by of ['bytes', 'lines']) {
    const body = by === 'bytes' ? '\tfirst\n second' : Array.from({ length: 2000 }, (_, i) => `line ${i}`).join('\n')
    const end = by === 'bytes' ? 21 : 2019
    const suffix = by === 'bytes' ? ' (50.0KB limit)' : ''
    const notice = `[Showing lines 20-${end} of 3000${suffix}. Use offset=${end + 1} to continue.]`
    const recorded = `${body}\n\n${notice}`
    const details = { ledger, truncation: cut(body, { truncatedBy: by }) }
    const source = readOutput(call({ path: 'a', offset: 20, limit: 2500 }, recorded, details))!
    assert.equal(source.body, body)
    assert.equal(source.notice, notice)
    assert.equal(source.startLine, 20)
    assert.equal(source.endLine, end)
    assert.equal(source.lineCount, body.split('\n').length)
    assert.equal(
      readOutput(call({ path: 'a', offset: 20 }, recorded.replace(`20-${end}`, `20-${end + 1}`), details)),
      null
    )
    assert.equal(
      readOutput(
        call({ path: 'a', offset: 20 }, recorded, {
          ledger,
          truncation: cut(body, { truncatedBy: by, outputBytes: 1 })
        })
      ),
      null
    )
    assert.equal(readOutput(call({ path: 'a', offset: 20 }, recorded)), null)
  }
})

test('unverified notices, special results, and errors retain original-output fallback', () => {
  const image: ChatContent = { type: 'image', mimeType: 'image/png', data: 'image' }
  for (const output of [
    [text('Read image file [image/png]'), image],
    [text('Read image file [image/png]\nprocessing failed')],
    [text('Binary file: a (123 bytes)')],
    [text('[Line 1 is 70.0KB, exceeds 50.0KB limit. Use bash to inspect]')]
  ])
    assert.equal(readOutput(call({ path: 'a' }, output, { ledger })), null)
  const lookalike = 'source\n\n[9 more lines in file. Use offset=2 to continue.]'
  assert.equal(readOutput(call({ path: 'a' }, lookalike, { ledger })), null)
  assert.equal(readOutput(call({ path: 'a', limit: 1 }, lookalike)), null)
  const bracketedSource = 'source\n[an ordinary source line]'
  assert.equal(readOutput(call({ path: 'a' }, bracketedSource))!.body, bracketedSource)
  const step = call({ path: 'a', offset: 8.9 }, 'error', { ledger })
  step.status = 'error'
  step.result!.isError = true
  assert.equal(readOutput(step), null)
  assert.equal(toolHeader(step, '/actual').path, ledger.path)
  assert.equal(toolHeader(step, '/actual').line, 8)
  step.result = null
  step.status = 'running'
  step.output = []
  assert.equal(readOutput(step), null)
  assert.equal(toolHeader(step, '/actual').line, 8)
  step.output = [text('partial output')]
  assert.equal(readOutput(step), null)
  for (const offset of [NaN, Infinity, '3', Number.MAX_SAFE_INTEGER + 1])
    assert.equal(readStartLine({ offset }), undefined)
})
