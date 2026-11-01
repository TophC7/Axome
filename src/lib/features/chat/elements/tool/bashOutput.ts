import type { ToolStep } from '../../turns'
import type { ChatContent } from '$lib/types/backend'
import { pathText } from './payload'

function fullOutputPath(details: unknown): string | null {
  if (!details || typeof details !== 'object' || !('fullOutputPath' in details)) return null
  const path = details.fullOutputPath
  return pathText(path) && path.trim() ? path : null
}

/** Only separate backend footers when their result metadata and paragraph position agree. */
export function bashOutput(call: ToolStep, limit: number | null = null) {
  const path = fullOutputPath(call.result?.details)
  let status: string | null = null
  let notice: string | null = null
  let parts = call.output
  const last = parts.at(-1)
  if (last?.type === 'text') {
    let text = last.text
    if (call.status === 'error') {
      const footer = text.match(
        /(?:^|\n\n)(Command exited with code [1-9]\d*|Command timed out after (?:\d+(?:\.\d+)?) seconds|Command aborted|Command terminated without an exit code)$/
      )
      if (footer && text.endsWith(footer[1])) {
        status = footer[1]
        text = text.slice(0, footer.index)
      }
    }
    // A matching path is evidence this is axo's notice, rather than a lookalike log line.
    if (path) {
      const footer = text.match(
        /(?:^|\n\n)(\[(?:Showing lines 1-\d+ and \d+-\d+ of \d+(?: \(50\.0KB limit\))?|Lines longer than 768 bytes were cut)\. Full output: [^\r\n]+\])$/
      )
      if (footer && text.endsWith(footer[1]) && footer[1].endsWith(`. Full output: ${path}]`)) {
        notice = footer[1].slice(1, -`. Full output: ${path}]`.length)
        text = text.slice(0, footer.index)
      }
    }
    if (text !== last.text) parts = [...parts.slice(0, -1), { type: 'text', text }]
  }

  let hidden = 0
  if (limit !== null) {
    const chunks = parts.map((part) => (part.type === 'text' && part.text ? part.text.split('\n') : []))
    const counts = chunks.map((lines) => lines.length - (lines.at(-1) === '' ? 1 : 0))
    hidden = Math.max(0, counts.reduce((sum, count) => sum + count, 0) - limit)
    let skip = hidden
    parts = parts.flatMap((part, index): ChatContent[] => {
      if (part.type === 'image') return [part]
      const from = Math.min(skip, counts[index])
      skip -= from
      return from < counts[index] ? [{ type: 'text', text: chunks[index].slice(from).join('\n') }] : []
    })
  }
  return { parts, status, notice, fullOutputPath: path, hidden }
}
