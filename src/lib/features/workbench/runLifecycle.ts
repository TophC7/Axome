import * as chats from '$lib/features/chat/registry'
import { restartTerminalProcess, stopTerminalProcess } from '$lib/features/terminal/lifecycle'
import { isChatTab, isTerminalTab, type RunTab } from '$lib/features/workbench/model'

export async function stopTabRun(tab: RunTab): Promise<void> {
  if (isChatTab(tab)) await chats.stop(tab)
  else if (isTerminalTab(tab)) await stopTerminalProcess(tab)
}

export async function restartTabRun(tab: RunTab): Promise<void> {
  if (isChatTab(tab)) await chats.restart(tab)
  else if (isTerminalTab(tab)) await restartTerminalProcess(tab)
}
