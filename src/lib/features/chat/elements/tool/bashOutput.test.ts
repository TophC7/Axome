import test from 'node:test'
import assert from 'node:assert/strict'
import type { ToolStep } from '../../turns'
import type { ChatContent } from '$lib/types/backend'
import { bashOutput } from './bashOutput'

function call(content: ChatContent[], isError = false, details: unknown = null): ToolStep {
  return {
    id: 'tool',
    entryId: 'entry',
    kind: 'tool',
    callId: 'bash-call',
    name: 'bash',
    arguments: { command: 'command' },
    status: isError ? 'error' : 'done',
    output: content,
    result: {
      kind: 'toolResult',
      callId: 'bash-call',
      toolName: 'bash',
      content,
      details,
      isError,
      durationMs: null
    }
  }
}

const text = (value: string): ChatContent => ({ type: 'text', text: value })

test('bash preview keeps twelve actual tail lines and all original whitespace when expanded', () => {
  const source = Array.from({ length: 15 }, (_, index) => `\trow ${index + 1}   value  `).join('\n') + '\n'
  const step = call([text(source)])
  const preview = bashOutput(step, 12)
  assert.equal(preview.hidden, 3)
  assert.deepEqual(preview.parts, [text(source.split('\n').slice(3).join('\n'))])
  assert.deepEqual(bashOutput(step).parts, [text(source)])
})

test('bash preserves real blank lines rather than spending preview on a phantom trailing line', () => {
  const source = 'first\n\n\tlast\n\n'
  const output = bashOutput(call([text(source)]), 3)
  assert.equal(output.hidden, 1)
  assert.deepEqual(output.parts, [text('\n\tlast\n\n')])
})

test('only an exact final failure paragraph on an error result becomes a semantic status', () => {
  for (const status of [
    'Command exited with code 7',
    'Command timed out after 0.25 seconds',
    'Command aborted',
    'Command terminated without an exit code'
  ]) {
    const source = `  logged text\t\n\n${status}`
    const output = bashOutput(call([text(source)], true))
    assert.equal(output.status, status)
    assert.deepEqual(output.parts, [text('  logged text\t')])
    assert.equal(bashOutput(call([text(source)])).status, null)
  }
  for (const source of [
    'log\nCommand aborted',
    'Command exited with code 0',
    'Command exited with code 7\n',
    'Command aborted\n\nmore log',
    'Command timed out after later seconds'
  ]) {
    const output = bashOutput(call([text(source)], true))
    assert.equal(output.status, null)
    assert.deepEqual(output.parts, [text(source)])
  }
})

test('truncation notice and status are independent of the local tail without losing source text', () => {
  const path = '/tmp/output with spaces.log'
  const body = Array.from({ length: 20 }, (_, index) => `line ${index + 1}`).join('\n')
  for (const notice of [
    `[Showing lines 1-800 and 801-2001 of 2001. Full output: ${path}]`,
    `[Showing lines 1-8 and 16-20 of 20 (50.0KB limit). Full output: ${path}]`,
    `[Lines longer than 768 bytes were cut. Full output: ${path}]`
  ]) {
    const source = `${body}\n\n${notice}\n\nCommand exited with code 1`
    const output = bashOutput(call([text(source)], true, { fullOutputPath: path }), 12)
    assert.equal(output.notice, notice.slice(1, -`. Full output: ${path}]`.length))
    assert.equal(output.status, 'Command exited with code 1')
    assert.equal(output.hidden, 8)
    assert.deepEqual(output.parts, [text(body.split('\n').slice(8).join('\n'))])
    assert.equal(output.fullOutputPath, path)
  }
})

test('lookalike truncation logs stay in the source unless metadata path and paragraph match', () => {
  const notice = '[Lines longer than 768 bytes were cut. Full output: /tmp/actual.log]'
  for (const details of [null, {}, { fullOutputPath: '/tmp/other.log' }, { fullOutputPath: 12 }]) {
    const source = `log\n\n${notice}`
    const output = bashOutput(call([text(source)], false, details))
    assert.equal(output.notice, null)
    assert.deepEqual(output.parts, [text(source)])
  }
  const source = `log\n${notice}`
  assert.equal(bashOutput(call([text(source)], false, { fullOutputPath: '/tmp/actual.log' })).notice, null)
  for (const path of ['', '/tmp/invalid\npath', '/tmp/invalid\u0000path']) {
    assert.equal(bashOutput(call([], false, { fullOutputPath: path })).fullOutputPath, null)
  }
})

test('image parts survive tail preview, including images after status-like text', () => {
  const image: ChatContent = { type: 'image', mimeType: 'image/png', data: 'image' }
  const source = Array.from({ length: 15 }, (_, index) => `line ${index}`).join('\n')
  const output = bashOutput(call([image, text(source), image]), 12)
  assert.equal(output.hidden, 3)
  assert.deepEqual(output.parts, [image, text(source.split('\n').slice(3).join('\n')), image])
  const afterImage = bashOutput(call([text('Command aborted'), image], true))
  assert.equal(afterImage.status, null)
  assert.deepEqual(afterImage.parts, [text('Command aborted'), image])
})
