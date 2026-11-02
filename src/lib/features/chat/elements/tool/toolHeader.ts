import type { StatusTone } from '$lib/components/ui/status-dot'
import { formatDuration } from '$lib/features/chat/format'
import { contentLines } from '$lib/features/chat/lineDiff'
import type { ToolStep } from '$lib/features/chat/turns'
import { isPathWithin, splitRemotePath } from '$lib/utils/paths'
import { readOutput, readPath, readStartLine } from './readOutput'
import { editDiff, integer, record } from './payload'

export type ToolHeader = {
  name: string
  summary: string
  path: string | null
  line: number | undefined
  pathLabel: string
  command: string | null
  /** Edit line counts, shown as git's net line change. */
  lines: { added: number; removed: number } | null
  meta: string
  tone: StatusTone
  status: { label: string; tone: 'danger' | 'working' } | null
}

export function toolHeader(call: ToolStep, folderPath: string): ToolHeader {
  const args = record(call.arguments)
  const search = call.name === 'grep' || call.name === 'find'
  const path =
    call.name === 'read'
      ? readPath(call)
      : search
        ? typeof args.path === 'string' && args.path
          ? args.path
          : '.'
        : typeof args.path === 'string'
          ? args.path
          : null
  const root = path?.startsWith('axome://') ? folderPath : (splitRemotePath(folderPath)?.path ?? folderPath)
  const relative =
    path !== null && isPathWithin(root, path) ? path.slice(root.replace(/\/+$/, '').length + 1) || '.' : (path ?? '')
  let summary = ''
  let pathLabel = relative
  let command: string | null = null
  let detail = ''
  let lines: ToolHeader['lines'] = null
  const failed = call.status === 'error'
  let line: number | undefined
  switch (call.name) {
    case 'bash': {
      command = typeof args.command === 'string' ? args.command : ''
      break
    }
    case 'read': {
      const source = readOutput(call)
      if (source?.endLine !== undefined) {
        line = source.startLine
        pathLabel += `:${line}${source.endLine !== line ? `-${source.endLine}` : ''}`
      } else if (!source) {
        line = readStartLine(args)
        if (line !== undefined && args.offset != null) pathLabel += `:${line}`
      }
      break
    }
    case 'edit': {
      if (failed) break
      const diff = editDiff(call.result?.details)
      if (diff) lines = { added: diff.added, removed: diff.removed }
      const firstChanged = record(call.result?.details).first_changed_line
      if (integer(firstChanged) && firstChanged > 0) line = firstChanged
      break
    }
    case 'write':
      if (typeof args.content === 'string') detail = `${contentLines(args.content).length} lines`
      break
    case 'grep':
    case 'find':
      summary = typeof args.pattern === 'string' ? JSON.stringify(args.pattern) : ''
      break
    case 'ask': {
      const question = Array.isArray(args.questions) ? args.questions[0] : null
      summary =
        question !== null && typeof question === 'object' && typeof question.question === 'string'
          ? question.question
          : ''
      break
    }
    default:
      summary = JSON.stringify(call.arguments) ?? ''
  }
  return {
    name: call.name || 'tool',
    summary: summary.replace(/[\r\n]+/g, ' '),
    path,
    line,
    pathLabel,
    command,
    lines,
    meta: [failed ? '' : detail, call.result?.durationMs != null ? formatDuration(call.result.durationMs) : '']
      .filter(Boolean)
      .join(' · '),
    tone: failed ? 'danger' : call.status === 'running' ? 'working' : 'success',
    status: failed
      ? { label: 'Failed', tone: 'danger' }
      : call.status === 'running'
        ? { label: 'Running', tone: 'working' }
        : null
  }
}
