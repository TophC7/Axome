import test from 'node:test'
import assert from 'node:assert/strict'
import { splitMarkdownBlocks } from './markdownBlocks'

test('splitMarkdownBlocks preserves source across heading, paragraph, and code blocks', () => {
  const source = '# A\n\npara\n\n```ts\nx\n```\n'
  const blocks = splitMarkdownBlocks(source)
  assert.equal(blocks.length, 3)
  assert.equal(blocks.join(''), source)
})

test('splitMarkdownBlocks keeps reference definitions in one document', () => {
  const source = '# A\n\nA [link][x].\n\n[x]: https://e.x\n'
  assert.deepEqual(splitMarkdownBlocks(source), [source])
})
