import type { StatusTone } from '$lib/components/ui/status-dot'
import type { ChatEntry, ChatToolStatus } from '$lib/types/backend'
import { isPathWithin, splitRemotePath } from '$lib/utils/paths'

export type ToolEntry = Extract<ChatEntry, { kind: 'tool' }>

export function kindLabel(kind: string): string {
  switch (kind) {
    case 'execute':
      return 'run'
    case 'switch_mode':
      return 'switch mode'
    case 'read':
    case 'edit':
    case 'delete':
    case 'move':
    case 'search':
    case 'fetch':
    case 'think':
      return kind
    default:
      return 'tool'
  }
}

const previewKeys = ['command', 'cmd', 'path', 'file_path', 'filePath', 'pattern', 'query', 'url'] as const

export function toolPreviewField(input: unknown): { key: string; value: string } | null {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) return null
  for (const key of previewKeys) {
    if (!Object.hasOwn(input, key)) continue
    const value: unknown = Reflect.get(input, key)
    if (typeof value === 'string' && value.trim()) return { key, value }
  }
  return null
}

export function toolPreview(entry: ToolEntry, folderPath: string): string {
  const field = toolPreviewField(entry.input)
  if (!field) return entry.title
  // ACP locations and arguments use host paths even for a remote project.
  const root = field.value.startsWith('sworm://') ? folderPath : (splitRemotePath(folderPath)?.path ?? folderPath)
  if (!isPathWithin(root, field.value)) return field.value
  return field.value.slice(root.replace(/\/+$/, '').length + 1) || '.'
}

export function toolStatusTone(
  status: ChatToolStatus,
  awaiting: boolean
): { tone: StatusTone; label: string; pulse: boolean } {
  if (awaiting) return { tone: 'waiting', label: 'Awaiting approval', pulse: false }
  switch (status) {
    case 'pending':
      return { tone: 'idle', label: 'Pending', pulse: false }
    case 'in_progress':
      return { tone: 'working', label: 'Running', pulse: true }
    case 'completed':
      return { tone: 'success', label: 'Completed', pulse: false }
    case 'failed':
      return { tone: 'danger', label: 'Failed', pulse: false }
  }
}
