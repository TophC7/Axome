import test from 'node:test'
import assert from 'node:assert/strict'
import { diffRows, patchRows } from './lineDiff'

test('diffRows retains three context lines around a change and collapses the remaining lines', () => {
  const oldLines = Array.from({ length: 20 }, (_, index) => `line ${index + 1}`)
  const newLines = [...oldLines]
  newLines[9] = 'changed line 10'

  assert.deepEqual(diffRows(oldLines.join('\n'), newLines.join('\n')), {
    rows: [
      { kind: 'gap', count: 6 },
      { kind: 'ctx', text: 'line 7' },
      { kind: 'ctx', text: 'line 8' },
      { kind: 'ctx', text: 'line 9' },
      { kind: 'del', text: 'line 10' },
      { kind: 'add', text: 'changed line 10' },
      { kind: 'ctx', text: 'line 11' },
      { kind: 'ctx', text: 'line 12' },
      { kind: 'ctx', text: 'line 13' },
      { kind: 'gap', count: 7 }
    ],
    added: 1,
    removed: 1
  })
})

test('diffRows treats every line as an addition when oldText is null', () => {
  assert.deepEqual(diffRows(null, 'first\nsecond\n'), {
    rows: [
      { kind: 'add', text: 'first' },
      { kind: 'add', text: 'second' },
      { kind: 'add', text: '' }
    ],
    added: 3,
    removed: 0
  })
})

test('diffRows returns no rows for identical texts', () => {
  assert.deepEqual(diffRows('first\nsecond\n', 'first\nsecond\n'), { rows: [], added: 0, removed: 0 })
})

test('patchRows parses two hunks without headers or newline markers', () => {
  assert.deepEqual(
    patchRows(
      '--- a/file\n+++ b/file\n@@ -1,2 +1,2 @@\n first\n-old\n+new\n@@ -8 +8 @@\n-last\n+next\n\\ No newline at end of file\n'
    ),
    {
      rows: [
        { kind: 'gap', count: 0 },
        { kind: 'ctx', text: 'first' },
        { kind: 'del', text: 'old' },
        { kind: 'add', text: 'new' },
        { kind: 'gap', count: 0 },
        { kind: 'del', text: 'last' },
        { kind: 'add', text: 'next' }
      ],
      added: 2,
      removed: 2
    }
  )
})
