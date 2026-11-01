import { diffArrays } from 'diff'
import type { DiffRow } from '$lib/features/chat/lineDiff'
import { filePathToLanguage } from '$lib/features/editor/languageMap'
import { codeTokens } from '$lib/utils/syntaxHighlight'

export type DiffToken = { text: string; color?: string; changed?: boolean }

/*
 * OMP word-emphasis behavior: packages/tui/src/chrome/diff.ts and
 * crates/pi-diff/src/lib.rs. Word alignment uses jsdiff's Myers engine.
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
// Only true single-line removal/addition pairs receive word emphasis.
export function wordHighlights(rows: DiffRow[]): Map<DiffRow, DiffToken[]> {
  const highlights = new Map<DiffRow, DiffToken[]>()
  for (let index = 0; index < rows.length;) {
    if (rows[index].kind !== 'del') {
      index += 1
      continue
    }
    const start = index
    while (index < rows.length && rows[index].kind === 'del') index += 1
    const added = index
    while (index < rows.length && rows[index].kind === 'add') index += 1
    if (added !== start + 1 || index !== added + 1) continue
    const oldRow = rows[start]
    const newRow = rows[added]
    if (oldRow.kind === 'gap' || newRow.kind === 'gap') continue
    const oldWords = words(oldRow.text)
    const newWords = words(newRow.text)
    const oldRanges: Array<[number, number]> = []
    const newRanges: Array<[number, number]> = []
    let oldIndex = 0
    let newIndex = 0
    for (const part of diffArrays(
      oldWords.map((word) => word.value),
      newWords.map((word) => word.value)
    )) {
      const count = part.value.length
      if (part.removed) {
        oldRanges.push([oldWords[oldIndex].start, oldWords[oldIndex + count - 1].end])
        oldIndex += count
      } else if (part.added) {
        newRanges.push([newWords[newIndex].start, newWords[newIndex + count - 1].end])
        newIndex += count
      } else {
        oldIndex += count
        newIndex += count
      }
    }
    highlights.set(oldRow, changedTokens(oldRow.text, oldRanges))
    highlights.set(newRow, changedTokens(newRow.text, newRanges))
  }
  return highlights
}

// Match OMP crates/pi-diff/src/lib.rs word_parts/js_trim: Latin/IPA word
// runs, otherwise one non-whitespace code point. Compare cores, but keep each
// side's source offsets so spacing is never replaced by the other side's text.
function words(text: string): Array<{ value: string; start: number; end: number }> {
  return Array.from(
    text.matchAll(/[\w\u00ad\u00c0-\u00d6\u00d8-\u00f6\u00f8-\u02c6\u02c8-\u02d7\u02de-\u02ff\u1e00-\u1eff]+|[^\s]/gu),
    (match) => ({ value: match[0], start: match.index, end: match.index + match[0].length })
  )
}

function changedTokens(text: string, ranges: Array<[number, number]>): DiffToken[] {
  const tokens: DiffToken[] = []
  let offset = 0
  for (const [start, end] of ranges) {
    if (start > offset) tokens.push({ text: text.slice(offset, start) })
    tokens.push({ text: text.slice(start, end), changed: true })
    offset = end
  }
  if (offset < text.length) tokens.push({ text: text.slice(offset) })
  return tokens
}

export function codeParts(text: string, tokens?: DiffToken[]): { indent: string; tokens: DiffToken[] } {
  const indent = /^[ \t]*/.exec(text)?.[0] ?? ''
  let skip = indent.length
  const content: DiffToken[] = []
  const source: DiffToken[] = tokens ?? [{ text }]
  for (const token of source) {
    const value = token.text.slice(Math.min(skip, token.text.length))
    skip = Math.max(0, skip - token.text.length)
    if (!value) continue
    content.push({ ...token, text: value })
  }
  return { indent, tokens: content }
}

export async function contextHighlights(
  rows: DiffRow[],
  path: string,
  folderPath: string
): Promise<Map<number, DiffToken[]>> {
  const highlights = new Map<number, DiffToken[]>()
  if (!path || !rows.some((row) => row.kind === 'ctx')) return highlights
  const language = filePathToLanguage(folderPath, path)
  if (language === 'plaintext') return highlights
  for (let index = 0; index < rows.length;) {
    if (rows[index].kind !== 'ctx') {
      index += 1
      continue
    }
    const start = index
    const contents: string[] = []
    while (index < rows.length) {
      const row = rows[index]
      if (row.kind !== 'ctx') break
      contents.push(row.text)
      index += 1
    }
    const tokens = await codeTokens(contents.join('\n'), language)
    if (!tokens) return highlights
    for (let line = 0; line < contents.length; line += 1) {
      highlights.set(
        start + line,
        tokens[line].map((token) => ({ text: token.content, color: token.color }))
      )
    }
  }
  return highlights
}
