import { getActiveFolderPath } from '$lib/features/workbench/state.svelte'

export type SidebarView = 'sessions' | 'files' | 'git' | 'issues'
export type GitSidebarTab = 'graph' | 'stashes' | 'branches' | 'tags' | 'worktrees'

/** Rail column width; the title bar's logo cell matches it so the borders line up. */
export const RAIL_WIDTH_CLASS = 'w-9'

let sidebarWidth = $state(280)
let sidebarCollapsed = $state(false)
let sidebarView = $state<SidebarView>('files')
let gitSidebarTab = $state<GitSidebarTab>('graph')

/** Folder whose sidebar panel is on screen; null while collapsed or with no folder open. */
export function getShownSidebarFolder(): string | null {
  return sidebarCollapsed ? null : getActiveFolderPath()
}

/** Panel width plus its 1px ResizeDivider (`w-px`): the sidebar column the title-bar header cell spans. */
export function getSidebarColumnWidth(): number {
  return sidebarWidth + 1
}

export function getSidebarWidth(): number {
  return sidebarWidth
}

export function setSidebarWidth(width: number) {
  sidebarWidth = Math.max(220, Math.min(520, width))
}

export function isSidebarCollapsed(): boolean {
  return sidebarCollapsed
}

export function setSidebarCollapsed(collapsed: boolean) {
  sidebarCollapsed = collapsed
}

export function toggleSidebar() {
  sidebarCollapsed = !sidebarCollapsed
}

export function getSidebarView(): SidebarView {
  return sidebarView
}

export function setSidebarView(view: SidebarView) {
  sidebarView = view
}

export function getGitSidebarTab(): GitSidebarTab {
  return gitSidebarTab
}

export function setGitSidebarTab(tab: GitSidebarTab) {
  gitSidebarTab = tab
}

export function revealGitChanges() {
  sidebarCollapsed = false
  sidebarView = 'git'
}
