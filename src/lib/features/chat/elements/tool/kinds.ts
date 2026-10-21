import type { StatusTone } from '$lib/components/ui/status-dot'
import type { ChatToolStatus } from '$lib/types/backend'
import {
  ArrowRightLeftIcon,
  BrainIcon,
  FileTextIcon,
  GlobeIcon,
  PencilIcon,
  SearchIcon,
  SparklesIcon,
  SquareTerminalIcon,
  Trash2,
  WrenchIcon
} from '$lib/icons/lucideExports'

export function toolKindIcon(kind: string) {
  switch (kind) {
    case 'read':
      return FileTextIcon
    case 'edit':
      return PencilIcon
    case 'delete':
      return Trash2
    case 'move':
      return ArrowRightLeftIcon
    case 'search':
      return SearchIcon
    case 'execute':
      return SquareTerminalIcon
    case 'fetch':
      return GlobeIcon
    case 'think':
      return BrainIcon
    case 'switch_mode':
      return SparklesIcon
    default:
      return WrenchIcon
  }
}

export function toolKindVerb(kind: string): string {
  switch (kind) {
    case 'read':
      return 'Read'
    case 'edit':
      return 'Edit'
    case 'delete':
      return 'Delete'
    case 'move':
      return 'Move'
    case 'search':
      return 'Search'
    case 'execute':
      return 'Run'
    case 'fetch':
      return 'Fetch'
    case 'think':
      return 'Think'
    default:
      return 'Tool'
  }
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
