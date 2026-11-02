// Task surface presentation helpers.
//
// Pure display derivations used by the tab bar and presentation layer.
// Tab-opening and lifecycle orchestration live in
// $lib/features/tasks/service.svelte.ts.

import type { TaskTab } from '$lib/features/workbench/model'

export function getTaskTabTitle(tab: TaskTab): string {
  if (tab.status === 'exited' && tab.exitCode !== null && tab.exitCode !== 0) {
    return `${tab.label} (exit ${tab.exitCode})`
  }
  return tab.label
}
