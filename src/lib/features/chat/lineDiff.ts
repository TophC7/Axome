export type DiffLine = { kind: 'add' | 'del' | 'ctx'; text: string; lineNumber: number | null }
export type DiffRow = DiffLine | { kind: 'gap' }
export type DiffResult = { rows: DiffRow[]; added: number; removed: number }

export function contentLines(text: string): string[] {
  const lines = text.split('\n')
  if (lines[lines.length - 1] === '') lines.pop()
  return lines
}

// Match OMP's two-line context windows without adding outer collapse markers.
const CONTEXT_LINES = 2

function trimContext(rows: DiffRow[]): DiffRow[] {
  const kept: DiffRow[] = []
  let gap = false
  const append = (row: DiffRow) => {
    if (gap && kept.length) kept.push({ kind: 'gap' })
    kept.push(row)
    gap = false
  }
  for (let index = 0; index < rows.length;) {
    if (rows[index].kind === 'gap') {
      gap = true
      index += 1
    } else if (rows[index].kind !== 'ctx') {
      append(rows[index++])
    } else {
      const start = index
      while (index < rows.length && rows[index].kind === 'ctx') index += 1
      const afterChange = start > 0 && rows[start - 1].kind !== 'gap'
      const beforeChange = index < rows.length && rows[index].kind !== 'gap'
      for (let current = start; current < index; current += 1) {
        if ((afterChange && current < start + CONTEXT_LINES) || (beforeChange && current >= index - CONTEXT_LINES)) {
          append(rows[current])
        } else {
          gap = true
        }
      }
    }
  }
  return kept
}

function result(rows: DiffRow[]): DiffResult {
  let added = 0
  let removed = 0
  for (const row of rows) {
    if (row.kind === 'add') added += 1
    else if (row.kind === 'del') removed += 1
  }
  return { rows: trimContext(rows), added, removed }
}

/** Plain source lines (read output, written file content) numbered from `startLine`. */
export function sourceRows(lines: string[], startLine: number): DiffLine[] {
  return lines.map((text, index) => ({ kind: 'ctx', text, lineNumber: startLine + index }))
}

export function parseDisplayDiff(text: string): DiffResult {
  const rows: DiffRow[] = []
  for (const line of contentLines(text)) {
    // Consume exactly one legacy separator, leaving code indentation untouched.
    const numbered = /^([+ -])[ \t]*(\d+)\|(.*)$/.exec(line) ?? /^([+ -])[ \t]*(\d+)[ \t](.*)$/.exec(line)
    if (numbered) {
      rows.push({
        kind: numbered[1] === '+' ? 'add' : numbered[1] === '-' ? 'del' : 'ctx',
        lineNumber: Number(numbered[2]),
        text: numbered[3]
      })
    } else if (line.trim() === '' || line.trim() === '...' || line.trim() === '…') {
      rows.push({ kind: 'gap' })
    } else if (/^[+ -]/.test(line)) {
      rows.push({
        kind: line[0] === '+' ? 'add' : line[0] === '-' ? 'del' : 'ctx',
        lineNumber: null,
        text: line.slice(1)
      })
    }
  }
  return result(rows)
}

export function patchRows(patch: string): DiffResult {
  const rows: DiffRow[] = []
  let oldLine = 0
  let newLine = 0
  let oldRemaining = 0
  let newRemaining = 0
  let previousOldEnd: number | null = null
  let previousNewEnd: number | null = null
  for (const line of contentLines(patch)) {
    const hunk = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(line)
    if (hunk) {
      oldLine = Number(hunk[1])
      newLine = Number(hunk[3])
      if (previousOldEnd !== null && (oldLine !== previousOldEnd || newLine !== previousNewEnd)) {
        rows.push({ kind: 'gap' })
      }
      oldRemaining = hunk[2] === undefined ? 1 : Number(hunk[2])
      newRemaining = hunk[4] === undefined ? 1 : Number(hunk[4])
      previousOldEnd = oldLine + oldRemaining
      previousNewEnd = newLine + newRemaining
      continue
    }
    // File headers and other metadata are only metadata outside a hunk.
    if (oldRemaining <= 0 && newRemaining <= 0) continue
    if (line.startsWith('+')) {
      rows.push({ kind: 'add', text: line.slice(1), lineNumber: newLine++ })
      newRemaining -= 1
    } else if (line.startsWith('-')) {
      rows.push({ kind: 'del', text: line.slice(1), lineNumber: oldLine++ })
      oldRemaining -= 1
    } else if (line.startsWith(' ')) {
      rows.push({ kind: 'ctx', text: line.slice(1), lineNumber: oldLine++ })
      newLine += 1
      oldRemaining -= 1
      newRemaining -= 1
    }
  }
  return result(rows)
}

/*
 * Preview budgeting adapted from OMP packages/tui/src/render/render-utils.ts,
 * truncateDiffByHunk: retain changes before proportionally budgeting context.
 *
 * MIT License
 *
 * Copyright (c) 2025 Mario Zechner
 * Copyright (c) 2025-2026 Can Bölük
 * Copyright (c) 2026 Stencil Labs, Inc.
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */
export function previewDiff(
  rows: DiffRow[],
  maxLines = 40,
  maxHunks = 8
): { rows: DiffRow[]; hiddenLines: number; hiddenHunks: number } {
  const segments: Array<{ rows: DiffRow[]; isChange: boolean; isGap: boolean }> = []
  for (const row of rows) {
    const isChange = row.kind === 'add' || row.kind === 'del'
    const previous = segments[segments.length - 1]
    if (row.kind !== 'gap' && previous && !previous.isGap && previous.isChange === isChange) {
      previous.rows.push(row)
    } else {
      segments.push({ rows: [row], isChange, isGap: row.kind === 'gap' })
    }
  }
  const changes = segments.filter((segment) => segment.isChange)
  const totalHunks = changes.length
  if (rows.length <= maxLines && totalHunks <= maxHunks) {
    return { rows, hiddenLines: 0, hiddenHunks: 0 }
  }

  const changeLines = changes.reduce((sum, segment) => sum + segment.rows.length, 0)
  const kept: DiffRow[] = []
  let keptHunks = 0
  let keptSourceLines = 0
  if (changeLines > maxLines) {
    for (const segment of segments) {
      if (kept.length >= maxLines) break
      if (segment.isChange) {
        if (keptHunks >= maxHunks) break
        keptHunks += 1
      }
      kept.push(...segment.rows.slice(0, maxLines - kept.length))
    }
    keptSourceLines = kept.length
  } else {
    const contextBudget = maxLines - changeLines
    const contextLines = rows.length - changeLines
    const ratio = contextLines > 0 ? contextBudget / contextLines : 0
    let remainingContext = contextBudget
    for (let index = 0; index < segments.length; index += 1) {
      const segment = segments[index]
      if (segment.isChange) {
        if (keptHunks >= maxHunks) break
        keptHunks += 1
        kept.push(...segment.rows)
        keptSourceLines += segment.rows.length
        continue
      }
      if (remainingContext <= 0) continue
      const allowed = Math.min(remainingContext, Math.max(1, Math.floor(segment.rows.length * ratio)))
      const outputStart = kept.length
      let sourceAdded = 0
      if (contextLines <= contextBudget) {
        kept.push(...segment.rows)
        sourceAdded = segment.rows.length
      } else if (segment.isGap || segment.rows.length <= allowed) {
        kept.push(...segment.rows.slice(0, allowed))
        sourceAdded = allowed
      } else {
        const beforeChange = segments[index + 1]?.isChange
        const afterChange = segments[index - 1]?.isChange
        if (beforeChange && afterChange) {
          const sourceBudget = allowed >= 3 ? allowed - 1 : allowed
          const first = Math.ceil(sourceBudget / 2)
          const last = sourceBudget - first
          kept.push(...segment.rows.slice(0, first))
          if (allowed >= 3) kept.push({ kind: 'gap' })
          if (last > 0) kept.push(...segment.rows.slice(-last))
          sourceAdded = sourceBudget
        } else if (beforeChange) {
          kept.push(...segment.rows.slice(-allowed))
          sourceAdded = allowed
        } else if (afterChange) {
          kept.push(...segment.rows.slice(0, allowed))
          sourceAdded = allowed
        } else {
          sourceAdded = Math.min(allowed, 2)
          kept.push(...segment.rows.slice(0, sourceAdded))
        }
      }
      keptSourceLines += sourceAdded
      remainingContext -= kept.length - outputStart
    }
  }
  return {
    rows: kept,
    hiddenLines: Math.max(0, rows.length - keptSourceLines),
    hiddenHunks: Math.max(0, totalHunks - keptHunks)
  }
}
