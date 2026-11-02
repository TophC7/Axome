// Workbench state — the single global tab list.
//
// Every tab carries its own `folderPath`; the active tab decides which
// folder the sidebar and folder-scoped commands operate on.
// There is no project lifecycle: opening a folder either focuses one of
// its tabs or seeds a new tab page for it.

import { SvelteSet } from 'svelte/reactivity'
import { backend } from '$lib/api/backend'
import { platform } from '$lib/platform'
import { releaseFolder } from '$lib/features/folders/lifecycle'
import { filterExistingFolders, pushRecentFolder } from '$lib/features/folders/state.svelte'
import { notify } from '$lib/features/notifications/state.svelte'
import { getErrorMessage } from '$lib/utils/client-error'
import * as terminals from '$lib/features/terminal/registry'
import * as chats from '$lib/features/chat/registry'
import type {
  ChatStatus,
  CompareDiffSource,
  CompareFileSide,
  CompareSide,
  DiffSource,
  DiffTab,
  EpicTab,
  IssueTab,
  NewTab,
  PersistedTab,
  SessionStatus,
  Tab,
  TabId,
  TaskRunStatus,
  TaskTab,
  TerminalTab,
  TextTab,
  Workbench
} from '$lib/features/workbench/model'
import { canLockTab, compareSidesEqual, isTerminalTab, tabServer } from '$lib/features/workbench/model'
import { flushActiveGroups, getGroupRefs, isTabInert, restoreGroups, syncGroups } from './groups.svelte'
import {
  loadPersistedWorkbench,
  flushLocalWorkbench,
  persistedToTab,
  schedulePersistWorkbench,
  serializeWorkbench,
  tabToPersisted
} from '$lib/features/workbench/persistence'
import { clampTabInsertionToGroup, compactGroupTabs, computeTabInsertion, reorderWithinGroup } from './tabInsertion'
import {
  ensureTextFileSyncListeners,
  isTextSurfaceDirty,
  openTextFile,
  revealTextTab,
  type TextRevealTarget
} from '$lib/features/workbench/surfaces/text/service.svelte'
import { basename, isPathWithin, resolveProjectFile, splitRemotePath } from '$lib/utils/paths'

// MODULE STATE //
let workbench = $state<Workbench>({ tabs: [], activeTabId: null })
let activationSeq = $state(0)

// LIFO stack of recently closed tabs for Ctrl+Shift+T. In-memory only: a
// fresh launch has nothing to reopen beyond what restore hydrates. The cap
// applies per kind group: chats/sessions/shells and everything else.
const MAX_CLOSED_TABS = 20
let closedTabs = $state<PersistedTab[]>([])

// Most recently active tab per folder, so "Open Folder" on an already
// open folder lands where the user last was rather than on its last tab.
const lastActiveByFolder = new Map<string, TabId>()

// Until restore has consulted disk, commits must not persist or the
// seeded empty state would clobber the saved blob.
let restored = false
let pendingFocusTabId: TabId | null = null
const stagedTabIds = new SvelteSet<TabId>()
const folderOps = new Map<string, Promise<void>>()

function queueFolderOp(folderPath: string, op: () => Promise<void>): Promise<void> {
  const prev = folderOps.get(folderPath) ?? Promise.resolve()
  const next = prev.catch(() => {}).then(op)
  folderOps.set(folderPath, next)
  void next
    .finally(() => {
      if (folderOps.get(folderPath) === next) folderOps.delete(folderPath)
    })
    .catch(() => {})
  return next
}

function awaitFolderOps(): Promise<void> {
  return Promise.all(folderOps.values()).then(() => {})
}

/** Await queued claims/releases, repeat strict teardown, then durably save the closed tabs. */
export async function releaseClosedWorktreeFolders(roots: string[], folderPaths: string[]): Promise<void> {
  if (getTabs().some((tab) => roots.some((root) => isPathWithin(root, tab.folderPath)))) {
    throw new Error('Worktree still has open tabs. Close them before continuing.')
  }
  // Earlier best-effort teardown is not proof; repeat it below and require that attempt to succeed.
  await Promise.allSettled(folderPaths.map((folderPath) => folderOps.get(folderPath)))
  await Promise.all(folderPaths.map((folderPath) => queueFolderOp(folderPath, () => releaseFolder(folderPath, true))))
  persistWorkbench()
  await flushWorkbench()
  if (getTabs().some((tab) => roots.some((root) => isPathWithin(root, tab.folderPath)))) {
    throw new Error('A worktree tab opened during shutdown. Close it before continuing.')
  }
}

// Monotonic counter for "Untitled-N" labels on new untitled text tabs.
let untitledCounter = 0

// HELPERS //
export function generateTabId(): TabId {
  return `tab-${crypto.randomUUID()}`
}

let nextRevealNonce = 0
function generateRevealNonce(): number {
  nextRevealNonce += 1
  return nextRevealNonce
}

function diffSourcesEqual(a: DiffSource, b: DiffSource): boolean {
  if (a.kind !== b.kind) return false

  switch (a.kind) {
    case 'working':
      return (
        b.kind === 'working' && a.staged === b.staged && a.scopePath === b.scopePath && a.revealNonce === b.revealNonce
      )
    case 'commit':
      return b.kind === 'commit' && a.commitHash === b.commitHash
    case 'stash':
      return b.kind === 'stash' && a.stashIndex === b.stashIndex
    case 'range':
      return b.kind === 'range' && a.base === b.base && a.target === b.target
    case 'compare':
      return b.kind === 'compare' && compareSidesEqual(a.left, b.left) && compareSidesEqual(a.right, b.right)
    default: {
      const _exhaustive: never = a
      return _exhaustive
    }
  }
}

function findTab(tabId: TabId): Tab | undefined {
  return workbench.tabs.find((t) => t.id === tabId)
}

/**
 * Single choke point for layout mutation: reassigns the workbench so
 * `$derived` consumers see a new reference, updates backend folder
 * ownership, then schedules a debounced persist.
 */
function commit(next: { tabs?: Tab[]; activeTabId?: TabId | null } = {}, persist = true): void {
  const native = platform.native
  const previousFolders = new Set(workbench.tabs.map((tab) => tab.folderPath))
  const tabs = next.tabs && native ? compactGroupTabs(next.tabs) : (next.tabs ?? workbench.tabs)
  const nextFolders = new Set(tabs.map((tab) => tab.folderPath))
  const activeTabId = next.activeTabId === undefined ? workbench.activeTabId : next.activeTabId
  if (activeTabId !== workbench.activeTabId) activationSeq += 1
  workbench = { tabs: [...tabs], activeTabId }
  const active = activeTabId ? tabs.find((t) => t.id === activeTabId) : undefined
  if (active) lastActiveByFolder.set(active.folderPath, active.id)
  for (const folderPath of nextFolders) {
    if (!previousFolders.has(folderPath)) {
      void queueFolderOp(folderPath, () =>
        backend.folders.claim(folderPath).catch((e) => console.warn('Folder claim failed:', e))
      )
    }
  }
  for (const folderPath of previousFolders) {
    if (!nextFolders.has(folderPath)) void queueFolderOp(folderPath, () => releaseFolder(folderPath))
  }
  if (persist) persistWorkbench()
  if (restored) syncGroups()
}

export function persistWorkbench(): void {
  if (!restored) return
  schedulePersistWorkbench(() => {
    const native = platform.native
    const tabs = workbench.tabs.filter((tab) => !stagedTabIds.has(tab.id))
    const local = native ? tabs.filter((tab) => !tabServer(tab)) : tabs
    const activeTabId = workbench.activeTabId
    const active = tabs.find((tab) => tab.id === activeTabId)
    const activeServer = active ? tabServer(active) : null
    const groupTabs = activeServer ? tabs.filter((tab) => tabServer(tab) === activeServer) : []
    return {
      ...serializeWorkbench({
        tabs: local,
        activeTabId: activeTabId != null && stagedTabIds.has(activeTabId) ? (local[0]?.id ?? null) : activeTabId
      }),
      ...(native
        ? {
            groups: getGroupRefs(),
            ...(activeServer
              ? { activeGroup: { server: activeServer, index: groupTabs.findIndex((tab) => tab.id === activeTabId) } }
              : {})
          }
        : {})
    }
  })
}

export async function flushWorkbench(): Promise<void> {
  await flushLocalWorkbench()
  await flushActiveGroups()
}

/**
 * Insert and activate a tab.
 * - New tabs are transient: the first real tab opened replaces that
 *   folder's new tab page in-place.
 * - Same-folder tabs open directly next to the active tab to keep folder tabs grouped.
 * - Other-folder tabs open after that folder's existing tabs.
 */
function insertTab(tab: Tab): TabId {
  if (isTabInert(tab)) throw new Error('This server workbench is not controlled by this window')
  const placement = computeTabInsertion(workbench.tabs, workbench.activeTabId, tab)
  const tabs = [...workbench.tabs]
  if ('replaceIndex' in placement) {
    tabs[placement.replaceIndex] = tab
  } else {
    tabs.splice(clampTabInsertionToGroup(tabs, tab, placement.insertIndex), 0, tab)
  }
  commit({ tabs, activeTabId: tab.id })
  return tab.id
}

function updateTab(tabId: TabId, update: (tab: Tab) => Tab): void {
  const index = workbench.tabs.findIndex((tab) => tab.id === tabId)
  if (index === -1) return

  const updated = update(workbench.tabs[index])
  if (updated === workbench.tabs[index]) return

  const tabs = [...workbench.tabs]
  tabs[index] = updated
  commit({ tabs })
}

function pushClosedTab(snapshot: PersistedTab): void {
  if (snapshot.kind === 'chat' && snapshot.sessionId === null) return
  // Separate caps: editor and diff churn must not evict a conversation's
  // session id from the Sessions view.
  let sessions = 0
  let others = 0
  closedTabs = [snapshot, ...closedTabs].filter((tab) =>
    tab.kind === 'chat' || tab.kind === 'session' || tab.kind === 'shell'
      ? ++sessions <= MAX_CLOSED_TABS
      : ++others <= MAX_CLOSED_TABS
  )
}

// READS //
/** Bumps whenever the active tab changes; one-shot UI requests compare against it. */
export function getActivationSeq(): number {
  return activationSeq
}

export function getTabs(): Tab[] {
  return workbench.tabs
}

export function getActiveTab(): Tab | null {
  return workbench.activeTabId ? (findTab(workbench.activeTabId) ?? null) : null
}

/** File path of the active tab when it is a text editor. */
export function getActiveTextFilePath(): string | null {
  const tab = getActiveTab()
  return tab?.kind === 'text' ? tab.filePath : null
}

/** Preview replacement must never discard edits or overwrite a locked/promoted tab. */
export function getReplaceableTemporaryTab(
  tabId: TabId | null = workbench.activeTabId
): Exclude<Tab, TerminalTab> | null {
  const tab = tabId ? findTab(tabId) : undefined
  return tab &&
    'temporary' in tab &&
    tab.temporary &&
    !tab.locked &&
    !(tab.kind === 'text' && isTextSurfaceDirty(tab.id))
    ? tab
    : null
}

export function getActiveTabId(): TabId | null {
  return workbench.activeTabId
}

export function getActiveFolderPath(): string | null {
  return getActiveTab()?.folderPath ?? null
}

/** The active tab's id when it is a terminal tab. */
export function getActiveTerminalTabId(): TabId | null {
  const tab = getActiveTab()
  return tab && isTerminalTab(tab) ? tab.id : null
}

export function hasClosedTabs(): boolean {
  return closedTabs.length > 0
}

/** Find an existing live task tab by its source task id (singleton rerun). */
export function findTaskTabByTaskId(folderPath: string, taskId: string): TaskTab | null {
  return (
    workbench.tabs.find(
      (t): t is TaskTab =>
        t.kind === 'task' &&
        t.folderPath === folderPath &&
        t.taskId === taskId &&
        t.status !== 'exited' &&
        t.status !== 'failed'
    ) ?? null
  )
}

// ACTIVATION / ORDER //
export function setActiveTab(tabId: TabId): void {
  if (workbench.activeTabId === tabId || !findTab(tabId)) return
  commit({ activeTabId: tabId })
}

export function requestFocusTab(tabId: TabId, reveal?: TextRevealTarget | null): void {
  if (findTab(tabId)) {
    setActiveTab(tabId)
    if (reveal) revealTextTab(tabId, reveal)
  } else if (!restored) {
    pendingFocusTabId = tabId
  }
}

export function reorderTab(fromIndex: number, toIndex: number): void {
  const tabs = workbench.tabs
  if (fromIndex < 0 || toIndex < 0 || fromIndex >= tabs.length || toIndex >= tabs.length) return
  const next = reorderWithinGroup(tabs, fromIndex, toIndex)
  if (next) commit({ tabs: next })
}
/** Stage an imported tab without persisting it before the broker commits. */
export function stageTransferredTab(tab: Tab, targetIndex: number): void {
  if (findTab(tab.id)) throw new Error(`Tab ${tab.id} already exists in target window`)
  stagedTabIds.add(tab.id)
  const tabs = [...workbench.tabs]
  tabs.splice(clampTabInsertionToGroup(tabs, tab, Math.max(0, Math.min(targetIndex, tabs.length))), 0, tab)
  commit({ tabs, activeTabId: tab.id }, false)
}

/** Remove a transferred tab without close prompts, process teardown, or file-claim release. */
export function removeTransferredTab(tabId: TabId, persist = true): void {
  stagedTabIds.delete(tabId)
  const index = workbench.tabs.findIndex((tab) => tab.id === tabId)
  if (index < 0) return
  const tab = workbench.tabs[index]
  const tabs = workbench.tabs.filter((candidate) => candidate.id !== tabId)
  const activeTabId =
    workbench.activeTabId === tabId ? ((tabs[index] ?? tabs[index - 1])?.id ?? null) : workbench.activeTabId
  if (lastActiveByFolder.get(tab.folderPath) === tabId) lastActiveByFolder.delete(tab.folderPath)
  commit({ tabs, activeTabId }, persist)
}

/** Replace one server's block without closing its runs (controller handoff). */
export function replaceGroupTabs(server: string, replacement: Tab[], atIndex?: number, activeIndex = 0): void {
  const first = workbench.tabs.findIndex((tab) => tabServer(tab) === server)
  const existing = workbench.tabs.filter((tab) => tabServer(tab) === server)
  const tabs = workbench.tabs.filter((tab) => tabServer(tab) !== server)
  const index = Math.max(0, Math.min(atIndex ?? (first < 0 ? tabs.length : first), tabs.length))
  tabs.splice(index, 0, ...replacement)
  const wasActive = existing.some((tab) => tab.id === workbench.activeTabId)
  const activeTabId =
    wasActive || (!workbench.activeTabId && replacement.length)
      ? (replacement[activeIndex]?.id ?? replacement[0]?.id ?? tabs[index]?.id ?? tabs[index - 1]?.id ?? null)
      : workbench.activeTabId
  commit({ tabs, activeTabId })
}

export function finalizeTransferredTab(tabId: TabId): void {
  stagedTabIds.delete(tabId)
  persistWorkbench()
}

export function toggleTabLocked(tabId: TabId): void {
  // The UI hides Lock for non-lockable kinds; guarding here keeps a stale
  // blob or future caller from locking a tab the rest of the code assumes
  // is always unlocked.
  updateTab(tabId, (t) => (canLockTab(t) ? { ...t, locked: !t.locked } : t))
}

// FOLDER ENTRY //
/**
 * Open a folder: canonicalize and remember it, then replace the requesting
 * preview or focus the folder's last tab (seed a new tab page when absent).
 */
export async function openFolder(path: string, replaceTabId?: TabId): Promise<void> {
  let folderPath: string
  try {
    folderPath = (await backend.folders.resolve(path)).path
  } catch (error) {
    notify.error('Open folder failed', getErrorMessage(error))
    return
  }
  pushRecentFolder(folderPath)

  const tab = replaceTabId ? getReplaceableTemporaryTab(replaceTabId) : null
  if (tab) {
    if (tab.folderPath === folderPath) {
      setActiveTab(tab.id)
      return
    }
    const replacement: NewTab = { kind: 'new-tab', id: tab.id, folderPath, locked: false, temporary: true }
    if (isTabInert(tab) || isTabInert(replacement)) {
      notify.error('Open folder failed', 'This server workbench is not controlled by this window')
      return
    }
    const tabs = workbench.tabs
      .filter(
        (candidate) => candidate.id === tab.id || candidate.kind !== 'new-tab' || candidate.folderPath !== folderPath
      )
      .map((candidate) => (candidate.id === tab.id ? replacement : candidate))
    releaseTextFileClaim(tab)
    commit({ tabs, activeTabId: tab.id })
    return
  }

  const remembered = lastActiveByFolder.get(folderPath)
  const existing =
    (remembered && findTab(remembered)?.folderPath === folderPath ? remembered : undefined) ??
    workbench.tabs.findLast((t) => t.folderPath === folderPath)?.id
  if (existing) {
    setActiveTab(existing)
    return
  }
  openNewTab(folderPath)
}

/** Open `window.home_folder` as a New Tab; warns and leaves Home showing when it fails. */
export async function openHomeFolder(): Promise<void> {
  try {
    const home = (await backend.settings.getEffective()).settings.window.home_folder
    if (!home) return
    const folder = (await backend.folders.resolve(home)).path
    await openFolder(folder)
  } catch (error) {
    notify.warning('Home folder unavailable', getErrorMessage(error))
  }
}

// RESTORE //
/**
 * Hydrate the persisted tab list. Tabs whose folder no longer resolves are
 * dropped; session tabs come back dormant and start on first activation.
 * Returns true when there was nothing to restore: no snapshot, or an empty
 * one (the web server seeds every new workbench with an empty snapshot).
 */
export async function restoreWorkbench(): Promise<boolean> {
  const native = platform.native
  try {
    await ensureTextFileSyncListeners()
    const persisted = await loadPersistedWorkbench()
    if (persisted) {
      const folders = await filterExistingFolders([
        ...new Set(persisted.tabs.filter((t) => !splitRemotePath(t.folderPath)).map((t) => t.folderPath))
      ])
      const alive = new Set(folders)

      // Claims run before commit, so a redirect that lands mid-restore is parked in pendingFocusTabId and applied at commit.
      // Keep original indices for nearest-neighbour fallback after drops.
      const candidates: Array<{ tab: Tab; index: number }> = []
      for (const [index, entry] of persisted.tabs.entries()) {
        if (alive.has(entry.folderPath) || (native && splitRemotePath(entry.folderPath)))
          candidates.push({ tab: persistedToTab(entry, generateTabId()), index })
      }

      const results = await Promise.all(
        candidates.map(async (c) => ({
          c,
          redirect:
            native !== null &&
            c.tab.kind === 'text' &&
            c.tab.filePath != null &&
            !c.tab.gitRef &&
            (await native.files.claimFile(resolveProjectFile(c.tab.folderPath, c.tab.filePath), c.tab.id)).status ===
              'redirect'
        }))
      )
      const survivors = results
        .filter(({ c, redirect }) => {
          if (redirect && c.tab.kind === 'text') {
            notify.warning('Duplicate file tab dropped', `File ${c.tab.filePath} is open in another window.`)
          }
          return !redirect
        })
        .map(({ c }) => c)

      const active =
        survivors.find((candidate) => candidate.tab.id === pendingFocusTabId) ??
        survivors.find((candidate) => candidate.index >= persisted.activeTabIndex) ??
        survivors.findLast((candidate) => candidate.index < persisted.activeTabIndex)
      pendingFocusTabId = null
      commit({ tabs: survivors.map((candidate) => candidate.tab), activeTabId: active?.tab.id ?? null })
      await awaitFolderOps()
      if (native) {
        await restoreGroups(persisted.groups ?? [])
        const activeGroup = persisted.activeGroup
        if (activeGroup) {
          const tabs = workbench.tabs.filter((tab) => tabServer(tab) === activeGroup.server)
          const tab = tabs[activeGroup.index]
          if (tab) commit({ activeTabId: tab.id })
        }
      }
    }
    return persisted === null || (persisted.tabs.length === 0 && !persisted.groups?.length)
  } catch (error) {
    console.warn('Workbench restore failed, starting empty:', error)
    return false
  } finally {
    restored = true
    syncGroups()
    persistWorkbench()
  }
}

// CHAT TABS //
export function addChatTab(folderPath: string, init: { sessionId: string | null; title: string }): TabId {
  return insertTab({
    kind: 'chat',
    id: generateTabId(),
    folderPath,
    title: init.title,
    sessionId: init.sessionId,
    status: 'dormant',
    locked: false
  })
}

export function setChatTabSessionId(tabId: TabId, sessionId: string | null): void {
  updateTab(tabId, (t) => (t.kind === 'chat' && t.sessionId !== sessionId ? { ...t, sessionId } : t))
}

export function setChatTabTitle(tabId: TabId, title: string): void {
  if (!title) return
  updateTab(tabId, (t) => (t.kind === 'chat' && t.title !== title ? { ...t, title } : t))
}

export function setChatTabStatus(tabId: TabId, status: ChatStatus): void {
  updateTab(tabId, (t) => (t.kind === 'chat' && t.status !== status ? { ...t, status } : t))
}

// SESSION TABS //
export interface SessionTabInit {
  harnessId: string
  preset: string | null
  title: string
  sessionId: string | null
}

/** Add a dormant session tab; the mounted surface spawns its process. */
export function addSessionTab(folderPath: string, init: SessionTabInit): TabId {
  return insertTab({
    kind: 'session',
    id: generateTabId(),
    folderPath,
    title: init.title,
    harnessId: init.harnessId,
    preset: init.preset,
    runId: null,
    sessionId: init.sessionId,
    status: 'dormant',
    locked: false
  })
}

export function addShellTab(folderPath: string, init: { preset: string | null; title: string }): TabId {
  return insertTab({
    kind: 'shell',
    id: generateTabId(),
    folderPath,
    title: init.title,
    preset: init.preset,
    runId: null,
    status: 'dormant',
    locked: false
  })
}

export function setTerminalTabStatus(tabId: TabId, status: SessionStatus): void {
  updateTab(tabId, (t) => ((t.kind === 'session' || t.kind === 'shell') && t.status !== status ? { ...t, status } : t))
}

export function setTerminalTabTitle(tabId: TabId, title: string): void {
  if (!title) return
  updateTab(tabId, (t) => ((t.kind === 'session' || t.kind === 'shell') && t.title !== title ? { ...t, title } : t))
}
export function setTerminalTabRunId(tabId: TabId, runId: string | null): void {
  updateTab(tabId, (t) => ((t.kind === 'session' || t.kind === 'shell') && t.runId !== runId ? { ...t, runId } : t))
}
/**
 * Persist a newly minted run id before asking the backend to start it.
 * Durability is best effort: the failed snapshot is requeued by
 * `flushWorkbench`, and a lost run id only costs reattach after a restart.
 */
export async function persistTerminalTabRunId(tabId: TabId, runId: string): Promise<void> {
  setTerminalTabRunId(tabId, runId)
  await flushWorkbench().catch((error) => console.warn('Failed to persist terminal run id:', error))
}

export function setSessionTabSessionId(tabId: TabId, sessionId: string | null): void {
  updateTab(tabId, (t) => (t.kind === 'session' && t.sessionId !== sessionId ? { ...t, sessionId } : t))
}

/** Drop the tab's session id only if it still equals `expectedId`; a session id bound meanwhile wins. */
export function clearSessionTabSessionId(tabId: TabId, expectedId: string): void {
  updateTab(tabId, (t) => (t.kind === 'session' && t.sessionId === expectedId ? { ...t, sessionId: null } : t))
}

// TASK TABS //
export interface TaskTabInit {
  runId: string
  taskId: string
  activeFilePath: string | null
  label: string
  icon: string | null
  group: string | null
}

/**
 * Add a new task tab. Always creates a fresh tab; callers that want
 * singleton focus-on-rerun use `findTaskTabByTaskId` first.
 */
export function addTaskTab(folderPath: string, init: TaskTabInit): TabId {
  return insertTab({
    kind: 'task',
    id: generateTabId(),
    folderPath,
    runId: init.runId,
    attachOnly: false,
    taskId: init.taskId,
    activeFilePath: init.activeFilePath,
    label: init.label,
    icon: init.icon,
    group: init.group,
    status: 'starting',
    exitCode: null,
    locked: false
  })
}

export function setTaskTabStatus(tabId: TabId, status: TaskRunStatus, exitCode: number | null = null): void {
  updateTab(tabId, (t) =>
    t.kind === 'task' && (t.status !== status || t.exitCode !== exitCode) ? { ...t, status, exitCode } : t
  )
}

/**
 * Rebind a singleton task tab to a fresh runId for restart. Resets
 * lifecycle status and caches the latest label/icon in case the task
 * definition changed since the tab was created.
 */
export function resetTaskTabForRestart(
  tabId: TabId,
  nextRunId: string,
  latest: {
    activeFilePath: string | null
    label: string
    icon: string | null
    group: string | null
  }
): void {
  updateTab(tabId, (t) =>
    t.kind === 'task'
      ? {
          ...t,
          runId: nextRunId,
          attachOnly: false,
          activeFilePath: latest.activeFilePath,
          label: latest.label,
          icon: latest.icon,
          group: latest.group,
          status: 'starting',
          exitCode: null
        }
      : t
  )
  setActiveTab(tabId)
}

// CONTENT TABS //
/** Compare the content-bearing fields of two tabs (ignoring id/locked). */
function tabDataChanged(a: Tab, b: Tab): boolean {
  if (a.kind !== b.kind || a.folderPath !== b.folderPath) return true
  switch (a.kind) {
    case 'diff':
      return b.kind !== 'diff' || !diffSourcesEqual(a.source, b.source) || a.initialFile !== b.initialFile
    case 'text':
      return b.kind !== 'text' || a.filePath !== b.filePath || a.gitRef !== b.gitRef
    case 'issue':
      return b.kind !== 'issue' || a.issueId !== b.issueId
    case 'epic':
      return b.kind !== 'epic' || a.epicId !== b.epicId
    default:
      return true
  }
}

function releaseTextFileClaim(tab: Tab): void {
  const native = platform.native
  if (native && tab.kind === 'text' && tab.filePath != null && !tab.gitRef) {
    void native.files.releaseFile(resolveProjectFile(tab.folderPath, tab.filePath))
  }
}

/**
 * Generic content-tab helper. Handles the 3-phase pattern:
 * 1. Replace the single temporary tab of this kind (global, any folder)
 * 2. Reuse an existing persistent tab (optional update via `onReuse`)
 * 3. Create a new tab
 */
function addContentTab(
  kind: Tab['kind'],
  makeTab: (id: TabId) => Tab,
  temporary: boolean,
  matchPersistent?: (t: Tab) => boolean,
  onReuse?: (existing: Tab) => Tab,
  tabId?: TabId
): TabId {
  if (temporary) {
    const existingTemp = tabId
      ? workbench.tabs.find((t) => t.id === tabId && t.kind === kind && 'temporary' in t && t.temporary)
      : workbench.tabs.find((t) => t.kind === kind && 'temporary' in t && t.temporary)
    if (existingTemp) {
      const newTab = makeTab(existingTemp.id)
      if (isTabInert(newTab)) throw new Error('This server workbench is not controlled by this window')
      if (
        existingTemp.kind === 'text' &&
        (newTab.kind !== 'text' ||
          newTab.folderPath !== existingTemp.folderPath ||
          newTab.filePath !== existingTemp.filePath ||
          newTab.gitRef)
      ) {
        releaseTextFileClaim(existingTemp)
      }
      // Skip mutation when tab data hasn't changed (second click of a
      // double-click on the same file).
      if (!tabDataChanged(existingTemp, newTab)) {
        setActiveTab(existingTemp.id)
        return existingTemp.id
      }
      if (existingTemp.folderPath === newTab.folderPath) {
        commit({
          tabs: workbench.tabs.map((t) => (t.id === existingTemp.id ? newTab : t)),
          activeTabId: newTab.id
        })
        return newTab.id
      }

      const remainingTabs = workbench.tabs.filter((t) => t.id !== existingTemp.id)
      const placement = computeTabInsertion(remainingTabs, workbench.activeTabId, newTab)
      if ('replaceIndex' in placement) {
        remainingTabs[placement.replaceIndex] = newTab
      } else {
        remainingTabs.splice(clampTabInsertionToGroup(remainingTabs, newTab, placement.insertIndex), 0, newTab)
      }
      commit({ tabs: remainingTabs, activeTabId: newTab.id })
      return newTab.id
    }
  }

  if (matchPersistent) {
    const existing = workbench.tabs.find((t) => matchPersistent(t))
    if (existing) {
      const updated = onReuse ? onReuse(existing) : existing
      commit({
        tabs: updated === existing ? workbench.tabs : workbench.tabs.map((t) => (t.id === existing.id ? updated : t)),
        activeTabId: existing.id
      })
      return existing.id
    }
  }

  return insertTab(makeTab(tabId ?? generateTabId()))
}

export function addCommitTab(
  folderPath: string,
  commitHash: string,
  shortHash: string,
  message: string,
  initialFile: string | null = null,
  temporary = true
): TabId {
  return addContentTab(
    'diff',
    (id): DiffTab => ({
      kind: 'diff',
      id,
      folderPath,
      source: { kind: 'commit', commitHash, shortHash, message },
      initialFile,
      temporary,
      locked: false
    }),
    temporary,
    (t) =>
      t.kind === 'diff' &&
      t.folderPath === folderPath &&
      t.source.kind === 'commit' &&
      t.source.commitHash === commitHash &&
      !t.temporary,
    (t) => (t.kind === 'diff' && t.initialFile !== initialFile ? { ...t, initialFile } : t)
  )
}

export function addChangesTab(
  folderPath: string,
  staged: boolean,
  scopePath: string | null = null,
  initialFile: string | null = null,
  temporary = true
): TabId {
  return addContentTab(
    'diff',
    (id): DiffTab => ({
      kind: 'diff',
      id,
      folderPath,
      source: { kind: 'working', staged, scopePath, revealNonce: generateRevealNonce() },
      initialFile,
      temporary,
      locked: false
    }),
    temporary,
    (t) =>
      t.kind === 'diff' &&
      t.folderPath === folderPath &&
      t.source.kind === 'working' &&
      t.source.staged === staged &&
      t.source.scopePath === scopePath &&
      !t.temporary,
    (t) =>
      t.kind === 'diff' && t.source.kind === 'working'
        ? {
            ...t,
            source: { kind: 'working', staged, scopePath, revealNonce: generateRevealNonce() },
            initialFile
          }
        : t
  )
}

export function addStashTab(
  folderPath: string,
  stashIndex: number,
  message: string,
  initialFile: string | null = null,
  temporary = true
): TabId {
  return addContentTab(
    'diff',
    (id): DiffTab => ({
      kind: 'diff',
      id,
      folderPath,
      source: { kind: 'stash', stashIndex, message },
      initialFile,
      temporary,
      locked: false
    }),
    temporary,
    (t) =>
      t.kind === 'diff' &&
      t.folderPath === folderPath &&
      t.source.kind === 'stash' &&
      t.source.stashIndex === stashIndex &&
      !t.temporary,
    (t) => (t.kind === 'diff' && t.initialFile !== initialFile ? { ...t, initialFile } : t)
  )
}

export function addRangeTab(
  folderPath: string,
  base: string,
  baseLabel: string,
  target: string | null,
  targetLabel: string,
  temporary = true
): TabId {
  return addContentTab(
    'diff',
    (id): DiffTab => ({
      kind: 'diff',
      id,
      folderPath,
      source: { kind: 'range', base, baseLabel, target, targetLabel },
      initialFile: null,
      temporary,
      locked: false
    }),
    temporary,
    (t) =>
      t.kind === 'diff' &&
      t.folderPath === folderPath &&
      t.source.kind === 'range' &&
      t.source.base === base &&
      t.source.target === target &&
      !t.temporary
  )
}

/** Compare tabs are always persistent; a clipboard compare for the same file reuses its tab with the new text. Both sides live in folderPath. */
export function addCompareTab(folderPath: string, left: CompareSide, right: CompareFileSide): TabId {
  const source: CompareDiffSource = { kind: 'compare', left, right }
  const match = (t: Tab) =>
    t.kind === 'diff' &&
    t.folderPath === folderPath &&
    t.source.kind === 'compare' &&
    compareSidesEqual(t.source.right, right) &&
    (left.kind === 'clipboard' ? t.source.left.kind === 'clipboard' : compareSidesEqual(t.source.left, left))
  return addContentTab(
    'diff',
    (id): DiffTab => ({
      kind: 'diff',
      id,
      folderPath,
      source,
      initialFile: null,
      temporary: false,
      locked: false
    }),
    false,
    match,
    (t) => (t.kind === 'diff' ? { ...t, source } : t)
  )
}

export function addTextTab(folderPath: string, filePath: string, temporary = true, tabId?: TabId): TabId {
  return addContentTab(
    'text',
    (id): TextTab => ({
      kind: 'text',
      id,
      folderPath,
      filePath,
      fileName: basename(filePath),
      temporary,
      locked: false
    }),
    temporary,
    (t) => t.kind === 'text' && t.folderPath === folderPath && t.filePath === filePath && !t.gitRef && !t.temporary,
    undefined,
    tabId
  )
}

/**
 * Open a new unsaved text tab ("Untitled-N"). Multiple presses stack
 * rather than dedupe; the tab promotes to a real path on first save.
 */
export function addUntitledTextTab(folderPath: string): TabId {
  untitledCounter += 1
  return insertTab({
    kind: 'text',
    id: generateTabId(),
    folderPath,
    filePath: null,
    fileName: `Untitled-${untitledCounter}`,
    temporary: false,
    locked: false
  })
}

/**
 * Promote an unsaved text tab (filePath=null) to a real path after
 * save-as, or rename an existing text tab's target.
 */
export function renameTextTab(tabId: TabId, nextRelative: string, nextFolder?: string): void {
  updateTab(tabId, (t) =>
    t.kind === 'text'
      ? { ...t, filePath: nextRelative, fileName: basename(nextRelative), folderPath: nextFolder ?? t.folderPath }
      : t
  )
}

/** Open a read-only text tab showing a file at a specific git revision. */
export function addReadonlyTextTab(
  folderPath: string,
  filePath: string,
  gitRef: string,
  refLabel: string,
  temporary = true
): TabId {
  return addContentTab(
    'text',
    (id): TextTab => ({
      kind: 'text',
      id,
      folderPath,
      filePath,
      fileName: basename(filePath),
      temporary,
      locked: false,
      gitRef,
      refLabel
    }),
    temporary,
    (t) =>
      t.kind === 'text' && t.folderPath === folderPath && t.filePath === filePath && t.gitRef === gitRef && !t.temporary
  )
}

/** Focus the folder's new tab page, or create one. One new tab page per folder. */
export function openNewTab(folderPath: string): TabId {
  const existing = workbench.tabs.find((t) => t.kind === 'new-tab' && t.folderPath === folderPath)
  if (existing) {
    setActiveTab(existing.id)
    return existing.id
  }
  const tab: NewTab = { kind: 'new-tab', id: generateTabId(), folderPath, locked: false, temporary: true }
  return insertTab(tab)
}

export function addIssueTab(folderPath: string, issueId: string, title: string, temporary = true): TabId {
  return addContentTab(
    'issue',
    (id): IssueTab => ({ kind: 'issue', id, folderPath, issueId, title, temporary, locked: false }),
    temporary,
    (t) => t.kind === 'issue' && t.folderPath === folderPath && t.issueId === issueId,
    (existing) => (existing.kind === 'issue' && existing.title !== title ? { ...existing, title } : existing)
  )
}

/** Update the cached title on an existing issue tab. */
export function updateIssueTabTitle(folderPath: string, issueId: string, title: string): void {
  const tab = workbench.tabs.find((t) => t.kind === 'issue' && t.folderPath === folderPath && t.issueId === issueId)
  if (!tab) return
  updateTab(tab.id, (t) => (t.kind === 'issue' && t.title !== title ? { ...t, title } : t))
}

export function addEpicTab(folderPath: string, epicId: string, title: string, temporary = true): TabId {
  return addContentTab(
    'epic',
    (id): EpicTab => ({ kind: 'epic', id, folderPath, epicId, title, temporary, locked: false }),
    temporary,
    (t) => t.kind === 'epic' && t.folderPath === folderPath && t.epicId === epicId,
    (existing) => (existing.kind === 'epic' && existing.title !== title ? { ...existing, title } : existing)
  )
}

/** Update the cached title on an existing epic tab. */
export function updateEpicTabTitle(folderPath: string, epicId: string, title: string): void {
  const tab = workbench.tabs.find((t) => t.kind === 'epic' && t.folderPath === folderPath && t.epicId === epicId)
  if (!tab) return
  updateTab(tab.id, (t) => (t.kind === 'epic' && t.title !== title ? { ...t, title } : t))
}

// PROMOTION //
export function promoteTab(tabId: TabId): void {
  updateTab(tabId, (t) =>
    !('temporary' in t) || t.kind === 'new-tab' || !t.temporary ? t : { ...t, temporary: false }
  )
}

export function promoteTabWhenReady(pendingTabId: TabId | Promise<TabId> | null | undefined): void {
  if (!pendingTabId) return
  void Promise.resolve(pendingTabId)
    .then((tabId) => promoteTab(tabId))
    .catch(() => {})
}

// CLOSE / REOPEN //
export function closeTab(tabId: TabId): void {
  const index = workbench.tabs.findIndex((t) => t.id === tabId)
  if (index < 0) return
  const tab = workbench.tabs[index]
  if (tab.locked || isTabInert(tab)) return

  // Active local and remote tasks survive view teardown. Explicit close
  // stops them and intentionally omits them from reopen history.
  const snapshot = tab.kind === 'task' ? null : tabToPersisted(tab)
  if (snapshot) pushClosedTab(snapshot)

  if (isTerminalTab(tab)) terminals.dispose(tab.id)
  if (tab.kind === 'chat') chats.dispose(tab.id)
  releaseTextFileClaim(tab)

  const tabs = workbench.tabs.filter((t) => t.id !== tabId)
  let activeTabId = workbench.activeTabId
  if (activeTabId === tabId) {
    activeTabId = (tabs[index] ?? tabs[index - 1])?.id ?? null
  }
  if (lastActiveByFolder.get(tab.folderPath) === tabId) lastActiveByFolder.delete(tab.folderPath)
  commit({ tabs, activeTabId })
}

/** A deleted checkout must never be resurrected through closed-tab history. */
export function forgetClosedWorktreeTabs(rootPath: string): void {
  closedTabs = closedTabs.filter((tab) => !isPathWithin(rootPath, tab.folderPath))
  for (const folderPath of lastActiveByFolder.keys()) {
    if (isPathWithin(rootPath, folderPath)) lastActiveByFolder.delete(folderPath)
  }
}

/**
 * Re-add one closed snapshot through its kind's add* function. On failure it
 * goes back to its slot in history and the error propagates.
 */
export async function reopenClosedTab(snapshot: PersistedTab): Promise<TabId | null> {
  const index = closedTabs.indexOf(snapshot)
  if (index === -1) return null
  closedTabs = closedTabs.filter((tab) => tab !== snapshot)

  try {
    switch (snapshot.kind) {
      // Closing killed this run. Reopen resumes harness history under a fresh PTY id.
      case 'session':
        return addSessionTab(snapshot.folderPath, {
          harnessId: snapshot.harnessId,
          preset: snapshot.preset,
          title: snapshot.title,
          sessionId: snapshot.sessionId
        })
      case 'chat':
        return addChatTab(snapshot.folderPath, { sessionId: snapshot.sessionId, title: snapshot.title })
      case 'shell':
        return addShellTab(snapshot.folderPath, { preset: snapshot.preset, title: snapshot.title })
      case 'task':
        // Task snapshots never enter closedTabs; keep the persisted union exhaustive.
        return null
      case 'text':
        return snapshot.gitRef
          ? addReadonlyTextTab(
              snapshot.folderPath,
              snapshot.filePath,
              snapshot.gitRef,
              snapshot.refLabel ?? snapshot.gitRef,
              false
            )
          : await openTextFile(snapshot.folderPath, snapshot.filePath, { temporary: false })
      case 'diff':
        switch (snapshot.source.kind) {
          case 'working':
            return addChangesTab(
              snapshot.folderPath,
              snapshot.source.staged,
              snapshot.source.scopePath ?? null,
              snapshot.initialFile,
              false
            )
          case 'commit':
            return addCommitTab(
              snapshot.folderPath,
              snapshot.source.commitHash,
              snapshot.source.shortHash,
              snapshot.source.message,
              snapshot.initialFile,
              false
            )
          case 'stash':
            return addStashTab(
              snapshot.folderPath,
              snapshot.source.stashIndex,
              snapshot.source.message,
              snapshot.initialFile,
              false
            )
          case 'range':
            return addRangeTab(
              snapshot.folderPath,
              snapshot.source.base,
              snapshot.source.baseLabel,
              snapshot.source.target,
              snapshot.source.targetLabel,
              false
            )
          case 'compare':
            return addCompareTab(snapshot.folderPath, snapshot.source.left, snapshot.source.right)
          default: {
            const _exhaustive: never = snapshot.source
            return _exhaustive
          }
        }
      case 'new-tab':
        return openNewTab(snapshot.folderPath)
      case 'issue':
        return addIssueTab(snapshot.folderPath, snapshot.issueId, snapshot.title, false)
      case 'epic':
        return addEpicTab(snapshot.folderPath, snapshot.epicId, snapshot.title, false)
      default: {
        const _exhaustive: never = snapshot
        return _exhaustive
      }
    }
  } catch (e) {
    closedTabs = [...closedTabs.slice(0, index), snapshot, ...closedTabs.slice(index)]
    throw e
  }
}

/**
 * Pop the most recently closed tab and re-add it via the normal add*
 * function for that kind. Returns the new tab id, or null when the stack
 * is empty. Mirrors VSCode's Ctrl+Shift+T.
 */
export async function reopenLastClosedTab(): Promise<TabId | null> {
  const head = closedTabs[0]
  return head ? reopenClosedTab(head) : null
}
