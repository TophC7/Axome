<!--
  @component
  SessionsSidebar — the folder's open chats, sessions and shells, indexed history,
  and recently closed shortcuts not covered by either.
  The header's + starts a new one.
-->

<script lang="ts">
  import SidebarPanel from '$lib/features/app-shell/sidebar/SidebarPanel.svelte'
  import { IconButton, iconButtonVariants } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { SidebarRow } from '$lib/components/ui/sidebar-row'
  import {
    AlertTriangle,
    ChevronDown,
    ChevronRight,
    HistoryIcon,
    MessagesSquareIcon,
    Plus,
    RefreshCwIcon,
    TerminalIcon,
    X
  } from '$lib/icons/lucideExports'
  import type { ChatStatus, ChatTab, SessionStatus, SessionTab, ShellTab } from '$lib/features/workbench/model'
  import {
    getActiveTabId,
    getClosedChats,
    getClosedSessions,
    getClosedShells,
    getTabs,
    reopenClosedTab,
    setActiveTab
  } from '$lib/features/workbench/state.svelte'
  import { closeTabWithChecks } from '$lib/features/workbench/tabActions.svelte'
  import {
    countLiveAgents,
    historyRows,
    newChat,
    openChatSession,
    startSession
  } from '$lib/features/sessions/service.svelte'
  import { getSessionIndex, refreshSessionIndex } from './state.svelte'
  import SessionHistory from './SessionHistory.svelte'
  import { sessionBrand, svgUrl } from '$lib/features/sessions/harnesses/catalog'
  import { getLaunchOptions } from '$lib/features/sessions/harnesses/state.svelte'
  import { startShell } from '$lib/features/shells/service.svelte'
  import { getShellLaunchOptions } from '$lib/features/shells/state.svelte'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { notify } from '$lib/features/notifications/state.svelte'
  import { getErrorMessage } from '$lib/utils/client-error'

  let { folderPath }: { folderPath: string } = $props()

  const SESSION_TONE: Record<SessionStatus | ChatStatus, StatusTone> = {
    running: 'working',
    starting: 'waiting',
    failed: 'danger',
    exited: 'idle',
    dormant: 'idle',
    idle: 'idle',
    waiting: 'waiting'
  }

  let chats = $derived(getTabs().filter((t): t is ChatTab => t.kind === 'chat' && t.folderPath === folderPath))
  let sessions = $derived(getTabs().filter((t): t is SessionTab => t.kind === 'session' && t.folderPath === folderPath))
  let shells = $derived(getTabs().filter((t): t is ShellTab => t.kind === 'shell' && t.folderPath === folderPath))
  let openGroups = $derived([
    { kind: 'chat' as const, label: 'Chats', tabs: chats },
    { kind: 'session' as const, label: 'Sessions', tabs: sessions },
    { kind: 'shell' as const, label: 'Shells', tabs: shells }
  ])
  let history = $derived(historyRows(folderPath))
  let index = $derived(getSessionIndex(folderPath))
  let refreshing = $derived(index.loading || (index.snapshot?.sources.some((source) => source.refreshing) ?? false))
  let closed = $derived([
    ...getClosedChats(folderPath).filter(
      (snapshot) =>
        !snapshot.sessionId ||
        (!history.some((session) => session.source.kind === 'axo' && session.sessionId === snapshot.sessionId) &&
          !chats.some((chat) => chat.sessionId === snapshot.sessionId))
    ),
    ...getClosedSessions(folderPath).filter(
      (snapshot) =>
        !snapshot.sessionId ||
        (!history.some(
          (session) =>
            session.source.kind === 'harness' &&
            session.source.harnessId === snapshot.harnessId &&
            session.sessionId === snapshot.sessionId
        ) &&
          !sessions.some(
            (session) => session.harnessId === snapshot.harnessId && session.sessionId === snapshot.sessionId
          ))
    ),
    ...getClosedShells(folderPath)
  ])
  let activeTabId = $derived(getActiveTabId())
  let liveAgents = $derived(countLiveAgents(folderPath))
  let launchers = $derived(
    getLaunchOptions(folderPath)
      .filter((option) => option.status?.status === 'connected')
      .map(({ harness, preset, label }) => ({
        id: `${harness.id}:${preset ?? ''}`,
        harnessId: harness.id,
        preset,
        label,
        icon: svgUrl(harness.icon)
      }))
  )
  let shellLaunchers = $derived(getShellLaunchOptions(folderPath))

  let openCollapsed = $state({ chat: false, session: false, shell: false })
  let closedCollapsed = $state(false)
</script>

<SidebarPanel title="Sessions">
  {#snippet headerActions()}
    <IconButton tooltip="Refresh Sessions" disabled={refreshing} onclick={() => void refreshSessionIndex(folderPath)}>
      <RefreshCwIcon size={12} class={refreshing ? 'animate-spin' : ''} />
    </IconButton>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger aria-label="New session" class={iconButtonVariants()}>
        <Plus size={12} />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        <DropdownMenu.Item onSelect={() => newChat(folderPath)}>
          <MessagesSquareIcon size={14} />
          <span>New Chat</span>
        </DropdownMenu.Item>
        <DropdownMenu.Separator />
        {#each launchers as launcher (launcher.id)}
          <DropdownMenu.Item onSelect={() => startSession(folderPath, launcher.harnessId, launcher.preset)}>
            <HarnessIcon icon={launcher.icon} size={14} />
            <span>{launcher.label}</span>
          </DropdownMenu.Item>
        {/each}
        <DropdownMenu.Separator />
        {#each shellLaunchers as launcher (launcher.preset)}
          <DropdownMenu.Item onSelect={() => startShell(folderPath, launcher.preset)}>
            <TerminalIcon size={14} />
            <span>{launcher.label}</span>
          </DropdownMenu.Item>
        {/each}
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {/snippet}

  <div class="h-full overflow-y-auto">
    {#if liveAgents > 1}
      <Alert.Root variant="warning" layout="banner">
        <AlertTriangle size={13} class="mt-0.5 shrink-0" />
        <span>{liveAgents} agents share this working tree</span>
      </Alert.Root>
    {/if}
    {#if chats.length === 0 && sessions.length === 0 && shells.length === 0 && closed.length === 0 && history.length === 0}
      <SidebarRow variant="info" divider={false}>No chats, sessions or shells in this folder yet</SidebarRow>
      <SidebarRow variant="action" divider={false} onclick={() => newChat(folderPath)}>
        <MessagesSquareIcon size={12} />
        <span class="min-w-0 flex-1 truncate">Start Chat</span>
        <Plus size={12} class="shrink-0" />
      </SidebarRow>
      {#each launchers as launcher (launcher.id)}
        <SidebarRow
          variant="action"
          divider={false}
          onclick={() => startSession(folderPath, launcher.harnessId, launcher.preset)}
        >
          <HarnessIcon icon={launcher.icon} size={12} />
          <span class="min-w-0 flex-1 truncate">Start {launcher.label}</span>
          <Plus size={12} class="shrink-0" />
        </SidebarRow>
      {/each}
      {#each shellLaunchers as launcher (launcher.preset)}
        <SidebarRow variant="action" divider={false} onclick={() => startShell(folderPath, launcher.preset)}>
          <TerminalIcon size={12} />
          <span class="min-w-0 flex-1 truncate">Start {launcher.label}</span>
          <Plus size={12} class="shrink-0" />
        </SidebarRow>
      {/each}
    {/if}

    {#each openGroups as group (group.kind)}
      {#if group.tabs.length > 0}
        {@render section(
          group.label,
          group.tabs.length,
          openCollapsed[group.kind],
          () => (openCollapsed[group.kind] = !openCollapsed[group.kind])
        )}
        {#if !openCollapsed[group.kind]}
          <div class="border-t border-edge/30 bg-surface py-1">
            {#each group.tabs as session (session.id)}
              <div class="group/row relative">
                <SidebarRow
                  variant="leaf"
                  on="surface"
                  divider={false}
                  pressed={session.id === activeTabId}
                  class="pr-7"
                  title="{session.title} — {session.status}"
                  onclick={() => setActiveTab(session.id)}
                >
                  {#if session.kind === 'shell'}
                    <TerminalIcon size={12} />
                  {:else if session.kind === 'chat'}
                    <MessagesSquareIcon size={12} />
                  {:else}
                    {@const meta = sessionBrand(session.folderPath, session.harnessId)}
                    {#if meta.icon}<HarnessIcon icon={meta.icon} size={12} />{/if}
                  {/if}
                  <span class="min-w-0 flex-1 truncate {session.id === activeTabId ? 'text-bright' : 'text-fg'}">
                    {session.title}
                  </span>
                  <StatusDot tone={SESSION_TONE[session.status]} size="sm" />
                </SidebarRow>
                <div
                  class="absolute inset-y-0 right-1 flex items-center opacity-0 group-hover/row:opacity-100 group-has-[:focus-visible]/row:opacity-100"
                >
                  <IconButton
                    tooltip={session.kind === 'chat'
                      ? 'Close chat'
                      : session.kind === 'shell'
                        ? 'Close shell'
                        : 'Close session'}
                    onclick={(e: MouseEvent) => {
                      e.stopPropagation()
                      void closeTabWithChecks(session.id)
                    }}
                  >
                    <X size={11} />
                  </IconButton>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {/if}
    {/each}

    {#key folderPath}
      <SessionHistory {folderPath} rows={history} {section} />
    {/key}

    {#if closed.length > 0}
      {@render section('Recently closed', closed.length, closedCollapsed, () => (closedCollapsed = !closedCollapsed))}
      {#if !closedCollapsed}
        <div class="border-t border-edge/30 bg-surface py-1">
          {#each closed as snapshot (snapshot)}
            <SidebarRow
              variant="leaf"
              on="surface"
              divider={false}
              title={snapshot.kind === 'chat' && snapshot.sessionId
                ? 'Reopen and resume this chat'
                : snapshot.kind === 'session' && snapshot.sessionId
                  ? 'Reopen and resume this conversation'
                  : 'Reopen (starts fresh)'}
              onclick={() => {
                if (snapshot.kind === 'chat' && snapshot.sessionId) {
                  openChatSession(folderPath, snapshot.sessionId, snapshot.title)
                  return
                }
                void reopenClosedTab(snapshot).catch((error) =>
                  notify.error(
                    snapshot.kind === 'chat'
                      ? 'Reopen chat failed'
                      : snapshot.kind === 'shell'
                        ? 'Reopen shell failed'
                        : 'Reopen session failed',
                    getErrorMessage(error)
                  )
                )
              }}
            >
              {#if snapshot.kind === 'shell'}
                <TerminalIcon size={12} class="opacity-60" />
              {:else if snapshot.kind === 'chat'}
                <MessagesSquareIcon size={12} class="opacity-60" />
              {:else}
                {@const meta = sessionBrand(snapshot.folderPath, snapshot.harnessId)}
                {#if meta.icon}<HarnessIcon icon={meta.icon} size={12} class="opacity-60" />{/if}
              {/if}
              <span class="min-w-0 flex-1 truncate text-muted">{snapshot.title}</span>
              {#if (snapshot.kind === 'session' || snapshot.kind === 'chat') && snapshot.sessionId}
                <HistoryIcon size={11} class="shrink-0 text-subtle" />
              {/if}
            </SidebarRow>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</SidebarPanel>

{#snippet section(label: string, count: number, collapsed: boolean, toggle: () => void)}
  <SidebarRow variant="section" onclick={toggle} aria-expanded={!collapsed}>
    {#if collapsed}
      <ChevronRight size={12} class="shrink-0 text-muted" />
    {:else}
      <ChevronDown size={12} class="shrink-0 text-muted" />
    {/if}
    <span class="min-w-0 flex-1 truncate text-fg">{label}</span>
    <span class="shrink-0 font-mono text-2xs text-subtle">{count}</span>
  </SidebarRow>
{/snippet}
