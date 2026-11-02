import type { TabId } from '$lib/features/workbench/model'

export const DND_MIME = {
  AXOME_ITEM: 'application/vnd.axome.item+json',
  AXOME_TAB: 'application/vnd.axome.kind.tab',
  AXOME_WORKBENCH: 'application/vnd.axome.kind.workbench',
  AXOME_FILE: 'application/vnd.axome.kind.file',
  AXOME_GIT_CHANGE: 'application/vnd.axome.kind.git-change',
  TEXT: 'text/plain',
  URI_LIST: 'text/uri-list',
  FILES: 'Files'
} as const

export interface TabDragItem {
  kind: 'tab'
  tabId: TabId
  sourceWindowLabel?: string
}

/** A window's whole server group, dragged by its server tab. */
export interface WorkbenchDragItem {
  kind: 'workbench'
  server: string
  workbenchId: string
  sourceWindowLabel: string
}

export interface FileDragItem {
  kind: 'file'
  path: string
  isDir: boolean
  folderPath: string
}

export interface GitChangeDragItem {
  kind: 'git-change'
  path: string
  staged: boolean
  folderPath: string
}

export interface OsFilesDragItem {
  kind: 'os-files'
  paths: string[]
}

export type AxomeDragKind = TabDragItem | WorkbenchDragItem | FileDragItem | GitChangeDragItem | OsFilesDragItem

export interface DragPayload {
  items: AxomeDragKind[]
  source: 'internal' | 'external'
}

export function parsePayload(raw: string | null | undefined): DragPayload | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as unknown
    return isDragPayload(parsed) ? parsed : null
  } catch {
    return null
  }
}

export function stampDataTransfer(dataTransfer: DataTransfer, payload: DragPayload): void {
  dataTransfer.setData(DND_MIME.AXOME_ITEM, JSON.stringify(payload))
  if (payload.items.some((item) => item.kind === 'tab')) {
    dataTransfer.setData(DND_MIME.AXOME_TAB, '')
  }
  if (payload.items.some((item) => item.kind === 'workbench')) {
    dataTransfer.setData(DND_MIME.AXOME_WORKBENCH, '')
  }
  if (payload.items.some((item) => item.kind === 'file')) {
    dataTransfer.setData(DND_MIME.AXOME_FILE, '')
  }
  if (payload.items.some((item) => item.kind === 'git-change')) {
    dataTransfer.setData(DND_MIME.AXOME_GIT_CHANGE, '')
  }
  const text = payloadText(payload)
  if (text) {
    dataTransfer.setData(DND_MIME.TEXT, text)
  }
}

export function dragTypes(event: DragEvent): readonly string[] {
  return Array.from(event.dataTransfer?.types ?? [])
}

function payloadText(payload: DragPayload): string {
  const first = payload.items[0]
  if (!first) return ''
  // Deliberately skip text/plain for tab drags. Monaco's drop handler reads
  // text/plain and inserts it at the caret, so stamping the tab id here
  // would make "tab-xxxx-N$0" appear inside the editor on drop.
  if (first.kind === 'tab') return ''
  if (first.kind === 'file' || first.kind === 'git-change') return first.path
  if (first.kind === 'os-files') return first.paths[0] ?? ''
  return ''
}

function isDragPayload(value: unknown): value is DragPayload {
  if (!value || typeof value !== 'object') return false
  const payload = value as Partial<DragPayload>
  if (payload.source !== 'internal' && payload.source !== 'external') return false
  if (!Array.isArray(payload.items)) return false
  return payload.items.every(isDragItem)
}

function isDragItem(value: unknown): value is AxomeDragKind {
  if (!value || typeof value !== 'object') return false
  const item = value as Partial<AxomeDragKind>
  if (item.kind === 'tab') {
    return (
      typeof item.tabId === 'string' &&
      (item.sourceWindowLabel === undefined || typeof item.sourceWindowLabel === 'string')
    )
  }
  if (item.kind === 'workbench') {
    return (
      typeof item.server === 'string' &&
      typeof item.workbenchId === 'string' &&
      typeof item.sourceWindowLabel === 'string'
    )
  }
  if (item.kind === 'file') {
    return typeof item.path === 'string' && typeof item.isDir === 'boolean' && typeof item.folderPath === 'string'
  }
  if (item.kind === 'git-change') {
    return typeof item.path === 'string' && typeof item.staged === 'boolean' && typeof item.folderPath === 'string'
  }
  if (item.kind === 'os-files') {
    return Array.isArray(item.paths) && item.paths.every((path) => typeof path === 'string')
  }
  return false
}
