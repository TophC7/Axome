import { backend } from '$lib/api/backend'
import type { ChatSessionTab, TabId } from '$lib/features/workbench/model'
import { setRunTabRunId, setRunTabStatus } from '$lib/features/workbench/state.svelte'
import { ChatController } from './controller.svelte'

const controllers = new Map<TabId, ChatController>()

export function getOrCreate(tab: ChatSessionTab): ChatController {
  let controller = controllers.get(tab.id)
  if (!controller) {
    controller = new ChatController(tab.id)
    controllers.set(tab.id, controller)
  }
  return controller
}

export function get(tabId: TabId): ChatController | undefined {
  return controllers.get(tabId)
}

export async function stop(tab: ChatSessionTab): Promise<void> {
  const controller = get(tab.id)
  if (controller) await controller.stop()
  else if (tab.runId) await backend.chat.stop(tab.runId)
}

export async function restart(tab: ChatSessionTab): Promise<void> {
  await stop(tab)
  setRunTabRunId(tab.id, null)
  setRunTabStatus(tab.id, 'dormant')
}

export function dispose(tabId: TabId): void {
  const controller = get(tabId)
  if (!controller) return
  void controller.stop().catch(() => {})
  controllers.delete(tabId)
}

export function release(tabId: TabId): void {
  get(tabId)?.release()
  controllers.delete(tabId)
}

export function releaseAll(): void {
  for (const controller of controllers.values()) controller.release()
  controllers.clear()
}
