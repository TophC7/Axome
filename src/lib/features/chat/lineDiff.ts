import { lcsLinePairs, MAX_REFINED_LINE_CHANGE_CELLS } from '../git/lineChanges'

export type DiffRow = { kind: 'add' | 'del' | 'ctx'; text: string } | { kind: 'gap'; count: number }

export function diffRows(
  oldText: string | null,
  newText: string,
  context = 3
): { rows: DiffRow[]; added: number; removed: number } {
  if (oldText === newText) return { rows: [], added: 0, removed: 0 }

  const newLines = newText.split('\n')
  if (oldText === null) {
    return { rows: newLines.map((text) => ({ kind: 'add', text })), added: newLines.length, removed: 0 }
  }

  const oldLines = oldText.split('\n')
  let prefix = 0
  while (prefix < oldLines.length && prefix < newLines.length && oldLines[prefix] === newLines[prefix]) prefix += 1
  let oldEnd = oldLines.length
  let newEnd = newLines.length
  while (oldEnd > prefix && newEnd > prefix && oldLines[oldEnd - 1] === newLines[newEnd - 1]) {
    oldEnd -= 1
    newEnd -= 1
  }

  const oldMid = oldLines.slice(prefix, oldEnd)
  const newMid = newLines.slice(prefix, newEnd)
  const pairs = oldMid.length * newMid.length <= MAX_REFINED_LINE_CHANGE_CELLS ? lcsLinePairs(oldMid, newMid) : []
  const fullRows: Array<{ kind: 'add' | 'del' | 'ctx'; text: string }> = []
  let added = 0
  let removed = 0
  for (let index = 0; index < prefix; index += 1) fullRows.push({ kind: 'ctx', text: oldLines[index] })

  let oldIndex = 0
  let newIndex = 0
  // The final boundary flushes any unmatched lines after the last LCS pair.
  for (const [oldMatch, newMatch] of [...pairs, [oldMid.length, newMid.length]]) {
    while (oldIndex < oldMatch) {
      fullRows.push({ kind: 'del', text: oldMid[oldIndex++] })
      removed += 1
    }
    while (newIndex < newMatch) {
      fullRows.push({ kind: 'add', text: newMid[newIndex++] })
      added += 1
    }
    if (oldMatch < oldMid.length) {
      fullRows.push({ kind: 'ctx', text: oldMid[oldIndex++] })
      newIndex += 1
    }
  }
  for (let index = oldEnd; index < oldLines.length; index += 1) fullRows.push({ kind: 'ctx', text: oldLines[index] })

  const rows: DiffRow[] = []
  for (let index = 0; index < fullRows.length;) {
    if (fullRows[index].kind !== 'ctx') {
      rows.push(fullRows[index++])
      continue
    }
    const start = index
    while (index < fullRows.length && fullRows[index].kind === 'ctx') index += 1
    const leadingEnd = start > 0 ? Math.min(start + context, index) : start
    const trailingStart = index < fullRows.length ? Math.max(leadingEnd, index - context) : index
    for (let kept = start; kept < leadingEnd; kept += 1) rows.push(fullRows[kept])
    if (trailingStart > leadingEnd) rows.push({ kind: 'gap', count: trailingStart - leadingEnd })
    for (let kept = trailingStart; kept < index; kept += 1) rows.push(fullRows[kept])
  }
  return { rows, added, removed }
}

export function patchRows(patch: string): { rows: DiffRow[]; added: number; removed: number } {
  const rows: DiffRow[] = []
  let added = 0
  let removed = 0
  for (const line of patch.split('\n')) {
    if (line.startsWith('--- ') || line.startsWith('+++ ')) continue
    if (line.startsWith('@@')) rows.push({ kind: 'gap', count: 0 })
    else if (line.startsWith('+')) {
      rows.push({ kind: 'add', text: line.slice(1) })
      added++
    } else if (line.startsWith('-')) {
      rows.push({ kind: 'del', text: line.slice(1) })
      removed++
    } else if (line.startsWith(' ')) rows.push({ kind: 'ctx', text: line.slice(1) })
  }
  return { rows, added, removed }
}
