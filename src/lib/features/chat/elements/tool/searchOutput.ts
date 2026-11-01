import type { ToolStep } from '../../turns'
import type { ChatContent } from '$lib/types/backend'
import { coversBody, integer, pathText, record, truncation } from './payload'

export type SearchLine = { text: string; link?: { path: string; line?: number; length: number } }
type SearchPart = { type: 'text'; lines: SearchLine[]; suffix: string } | Extract<ChatContent, { type: 'image' }>
export type SearchOutputModel = { parts: SearchPart[]; hidden: number }

const relativePath = (value: string): boolean =>
  pathText(value) &&
  !value.startsWith('/') &&
  !value.split('/').some((part) => part === '.' || part === '..' || part === '')

/** Recognize only the exact metadata-backed footer, never a bracketed filename or source line. */
function searchBody(
  text: string,
  kind: 'grep' | 'find',
  args: Record<string, unknown>,
  details: Record<string, unknown>
): string | null {
  const notices: string[] = []
  const reached = details[kind === 'grep' ? 'matchLimitReached' : 'resultLimitReached']
  if (reached != null) {
    const requested = args.limit ?? (kind === 'grep' ? 100 : 1000)
    if (
      !integer(reached) ||
      reached < 1 ||
      typeof requested !== 'number' ||
      !Number.isFinite(requested) ||
      reached !== Math.ceil(Math.max(kind === 'grep' ? 1 : 0, requested)) ||
      !Number.isSafeInteger(reached * 2)
    )
      return null
    notices.push(
      `${reached} ${kind === 'grep' ? 'matches' : 'results'} limit reached. Use limit=${reached * 2} for more, or refine pattern`
    )
  }
  const cut = details.truncation == null ? null : truncation(details.truncation)
  if (
    details.truncation != null &&
    (!cut || cut.truncatedBy !== 'bytes' || typeof cut.firstLineExceedsLimit !== 'boolean')
  )
    return null
  if (cut) notices.push('50.0KB limit reached')
  if (kind === 'grep' && details.linesTruncated === true)
    notices.push('Some lines truncated to 500 chars. Use read tool to see full lines')
  const suffix = notices.length ? `\n\n[${notices.join('. ')}]` : ''
  if (suffix && !text.endsWith(suffix)) return null
  const body = suffix ? text.slice(0, -suffix.length) : text
  if (cut && (!coversBody(cut, body) || (cut.firstLineExceedsLimit === true && (body !== '' || cut.outputLines !== 0))))
    return null
  return body
}

function linkLine(text: string, kind: 'grep' | 'find'): SearchLine {
  const row = text.endsWith('\n') ? text.slice(0, -1) : text
  if (kind === 'find') {
    if (row === 'No files found matching pattern') return { text }
    const path = row.endsWith('/') ? row.slice(0, -1) : row
    return relativePath(path) ? { text, link: { path: row, length: row.length } } : { text }
  }
  // Another viable delimiter may be part of either the filename or source. Leave that row untouched.
  const candidates = [...row.matchAll(/:([1-9]\d*): |-([1-9]\d*)- /g)].filter((match) =>
    relativePath(row.slice(0, match.index))
  )
  if (candidates.length !== 1) return { text }
  const match = candidates[0]
  const line = Number(match[1] ?? match[2])
  return integer(line) && line > 0
    ? { text, link: { path: row.slice(0, match.index), line, length: match.index + match[0].length - 2 } }
    : { text }
}

/** Annotate original text in place: no sorting, grouping, regex execution, or discarded unknown rows. */
export function searchOutput(call: ToolStep, limit: number | null = null): SearchOutputModel {
  const args = record(call.arguments)
  const kind = call.name === 'grep' || call.name === 'find' ? call.name : null
  const canLink =
    kind !== null &&
    call.status === 'done' &&
    typeof args.pattern === 'string' &&
    (args.path == null || args.path === '' || pathText(args.path)) &&
    call.output.length === 1
  let remaining = limit ?? Infinity
  let hidden = 0
  const parts: SearchPart[] = call.output.map((part): SearchPart => {
    if (part.type === 'image') return part
    const body = canLink && kind ? searchBody(part.text, kind, args, record(call.result?.details)) : null
    const source = body ?? part.text
    const rows = source.match(/[^\n]*\n|[^\n]+$/g) ?? []
    const shown = rows.slice(0, remaining)
    remaining = Math.max(0, remaining - shown.length)
    hidden += rows.length - shown.length
    let suffix = body !== null ? part.text.slice(body.length) : ''
    // The preview ends at this row, so retain the footer's original paragraph gap without an extra newline.
    if (suffix && shown.length < rows.length && shown.at(-1)?.endsWith('\n')) suffix = suffix.slice(1)
    return {
      type: 'text',
      lines: shown.map((row) => (body !== null && kind ? linkLine(row, kind) : { text: row })),
      suffix
    }
  })
  return { parts, hidden }
}
