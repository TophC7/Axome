import type { ChatEntry } from '$lib/types/backend'

export type TranscriptBlock = { kind: 'entry'; entry: ChatEntry } | { kind: 'tools'; id: string; entries: ChatEntry[] }

export function transcriptBlocks(entries: ChatEntry[]): TranscriptBlock[] {
  const blocks: TranscriptBlock[] = []
  let index = 0
  while (index < entries.length) {
    const start = index
    let tools = 0
    while (index < entries.length && (entries[index].kind === 'thought' || entries[index].kind === 'tool')) {
      if (entries[index].kind === 'tool') tools++
      index++
    }
    if (tools >= 2) {
      blocks.push({ kind: 'tools', id: `tools:${entries[start].id}`, entries: entries.slice(start, index) })
    } else if (index > start) {
      for (let i = start; i < index; i++) blocks.push({ kind: 'entry', entry: entries[i] })
    } else {
      blocks.push({ kind: 'entry', entry: entries[index++] })
    }
  }
  return blocks
}
