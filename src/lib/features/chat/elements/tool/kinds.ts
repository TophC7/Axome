import type { StatusTone } from '$lib/components/ui/status-dot'
import type { ToolStep } from '$lib/features/chat/turns'
import { isPathWithin, splitRemotePath } from '$lib/utils/paths'

export function kindLabel(name: string): string {
  return name || 'tool'
}

export function toolPreviewField(name: string, input: unknown): { key: string; value: string } | null {
  if (input === null || typeof input !== 'object' || Array.isArray(input)) return null
  const fields = input as Record<string, unknown>
  if (name === 'ask') {
    const first = Array.isArray(fields.questions) ? fields.questions[0] : null
    if (first && typeof first === 'object' && typeof first.question === 'string') {
      return { key: 'questions', value: first.question }
    }
    return null
  }
  const key = name === 'bash' ? 'command' : name === 'grep' || name === 'find' ? 'pattern' : 'path'
  return typeof fields[key] === 'string' ? { key, value: fields[key] } : null
}

export function toolPreview(call: ToolStep, folderPath: string): string {
  const field = toolPreviewField(call.name, call.arguments)
  if (!field) return ''
  if (field.key !== 'path') return field.value
  const root = field.value.startsWith('sworm://') ? folderPath : (splitRemotePath(folderPath)?.path ?? folderPath)
  return isPathWithin(root, field.value) ? field.value.slice(root.replace(/\/+$/, '').length + 1) || '.' : field.value
}

export function toolStatusTone(call: ToolStep): { tone: StatusTone; label: string } {
  if (call.result?.isError || call.status === 'error') return { tone: 'danger', label: 'Failed' }
  if (call.status === 'running') return { tone: 'working', label: 'Running' }
  return { tone: 'success', label: 'Completed' }
}
