import { TerminalManager } from './TerminalManager'
import { behaviorFor } from './behaviors'
import { createTerminalRegistry } from './terminalRegistry'
import type { TerminalTab } from '$lib/features/workbench/model'

export const {
  getOrCreate,
  get,
  detach,
  focus,
  release,
  dispose,
  releaseAll,
  exportTransferState,
  importTransferState
} = createTerminalRegistry<TerminalTab, TerminalManager>(
  'terminal',
  (tab) => tab.id,
  (tab) => new TerminalManager(tab.id, behaviorFor(tab))
)

export function rebind(tabId: string): void {
  dispose(tabId)
}
