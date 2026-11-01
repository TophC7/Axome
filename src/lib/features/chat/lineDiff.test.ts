import test from 'node:test'
import assert from 'node:assert/strict'
import { parseDisplayDiff, patchRows, previewDiff, type DiffLine, type DiffRow } from './lineDiff'

const row = (kind: DiffLine['kind'], lineNumber: number, text: string): DiffLine => ({ kind, lineNumber, text })

test('numbered display formats preserve indentation, blank code and literal ellipses', () => {
  const expected = {
    rows: [
      row('ctx', 8, '...'),
      row('ctx', 9, '\t...rest'),
      row('del', 10, '    old'),
      row('add', 10, '\tnew'),
      row('ctx', 11, ''),
      row('ctx', 12, '…')
    ],
    added: 1,
    removed: 1
  }
  assert.deepEqual(parseDisplayDiff('  8 ...\n  9 \t...rest\n- 10     old\n+ 10 \tnew\n 11 \n 12 …'), expected)
  assert.deepEqual(parseDisplayDiff('  8|...\n  9|\t...rest\n- 10|    old\n+ 10|\tnew\n 11|\n 12|…'), expected)
  assert.deepEqual(parseDisplayDiff(''), { rows: [], added: 0, removed: 0 })
})

test('display context merges four unchanged rows but gaps more distant changes', () => {
  for (const unchanged of [4, 5]) {
    const text = [
      '...',
      ' 1 leading',
      ' 2 leading',
      ' 3 leading',
      '-4 old',
      '+4 new',
      ...Array.from({ length: unchanged }, (_, index) => ` ${index + 5} context`),
      `+${unchanged + 5} inserted`,
      ` ${unchanged + 5} trailing`,
      ` ${unchanged + 6} trailing`,
      ` ${unchanged + 7} trailing`,
      '...'
    ].join('\n')
    const parsed = parseDisplayDiff(text)
    assert.equal(parsed.added, 2)
    assert.equal(parsed.removed, 1)
    assert.deepEqual(parsed.rows[0], row('ctx', 2, 'leading'))
    assert.deepEqual(parsed.rows.at(-1), row('ctx', unchanged + 6, 'trailing'))
    assert.equal(parsed.rows.filter((line) => line.kind === 'gap').length, unchanged === 4 ? 0 : 1)
    assert.equal(parsed.rows.filter((line) => line.kind === 'ctx' && line.text === 'context').length, 4)
  }
})

test('display gap markers separate disjoint regions without consuming numbered code', () => {
  assert.deepEqual(parseDisplayDiff('...\n-10 old\n+10 new\n...\n...\n 40 ...\n+41 next\n…'), {
    rows: [
      row('del', 10, 'old'),
      row('add', 10, 'new'),
      { kind: 'gap' },
      row('ctx', 40, '...'),
      row('add', 41, 'next')
    ],
    added: 2,
    removed: 1
  })
})

test('patchRows uses hunk coordinates, preserves header-like code and skips metadata outside hunks', () => {
  assert.deepEqual(
    patchRows(
      'diff --git a/file b/file\n--- a/file\n+++ b/file\n@@ -10,2 +20,2 @@\n first\n--- literal\n+++ literal\n@@ -40 +50 @@\n-last\n+next\n\\ No newline at end of file\n--- a/other\n+++ b/other\n'
    ),
    {
      rows: [
        row('ctx', 10, 'first'),
        row('del', 11, '-- literal'),
        row('add', 21, '++ literal'),
        { kind: 'gap' },
        row('del', 40, 'last'),
        row('add', 50, 'next')
      ],
      added: 2,
      removed: 2
    }
  )
  assert.deepEqual(patchRows('@@ -0,0 +1,2 @@\n+first\n+second\n'), {
    rows: [row('add', 1, 'first'), row('add', 2, 'second')],
    added: 2,
    removed: 0
  })
})

test('preview budgets context proportionally without dropping edits or counting synthetic gaps as source rows', () => {
  const context = (start: number, length: number) =>
    Array.from({ length }, (_, index) => row('ctx', start + index, `line ${start + index}`))
  const rows: DiffRow[] = [
    ...context(1, 10),
    row('add', 11, 'added'),
    ...context(12, 20),
    row('del', 32, 'removed'),
    ...context(33, 10)
  ]
  const preview = previewDiff(rows, 14)
  assert.equal(preview.rows.length, 14)
  assert.equal(preview.hiddenLines, 29)
  assert.equal(preview.hiddenHunks, 0)
  assert.deepEqual(
    preview.rows.filter((line) => line.kind === 'add' || line.kind === 'del'),
    [row('add', 11, 'added'), row('del', 32, 'removed')]
  )
  assert.deepEqual(preview.rows[0], row('ctx', 8, 'line 8'))
  assert.deepEqual(preview.rows.at(-1), row('ctx', 35, 'line 35'))
  assert.equal(preview.rows.filter((line) => line.kind === 'gap').length, 1)
})

test('preview limits contiguous change runs to eight and counts hidden source gap rows', () => {
  const rows: DiffRow[] = Array.from({ length: 9 }, (_, index) => [
    row('add', index + 1, 'added'),
    { kind: 'gap' } as const
  ]).flat()
  const preview = previewDiff(rows)
  assert.equal(preview.hiddenHunks, 1)
  assert.equal(preview.hiddenLines, 2)
  assert.equal(preview.rows.filter((line) => line.kind === 'add').length, 8)
})

test('preview caps an oversized change run while the full expansion model retains more than 400 lines', () => {
  const rows: DiffRow[] = Array.from({ length: 450 }, (_, index) => row('add', index + 1, `line ${index + 1}`))
  const preview = previewDiff(rows)
  assert.equal(preview.rows.length, 40)
  assert.equal(preview.hiddenLines, 410)
  assert.equal(preview.hiddenHunks, 0)
  assert.equal(previewDiff(rows, 450).rows, rows)
})
