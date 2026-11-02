<!--
  @component
  SessionsSidebar — two panes like Git: the folder's open sessions (Axo chats,
  harness CLIs and shells, in tab order) on top, indexed history below.
  The header's + starts a new one.
-->

<script lang="ts">
  import SidebarPanel from '$lib/features/app-shell/sidebar/SidebarPanel.svelte'
  import { IconButton, iconButtonVariants } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import * as Resizable from '$lib/components/ui/resizable'
  import { SidebarRow } from '$lib/components/ui/sidebar-row'
  import { AlertTriangle, MessagesSquareIcon, Plus, TerminalIcon, X } from '$lib/icons/lucideExports'
  import type { ChatStatus, ChatTab, SessionStatus, SessionTab, ShellTab } from '$lib/features/workbench/model'
  import { getActiveTabId, getTabs, setActiveTab } from '$lib/features/workbench/state.svelte'
  import { closeTabWithChecks } from '$lib/features/workbench/tabActions.svelte'
  import { countLiveAgents, historyRows, newChat, startSession } from '$lib/features/sessions/service.svelte'
  import SessionHistory from './SessionHistory.svelte'
  import { sessionBrand, svgUrl } from '$lib/features/sessions/harnesses/catalog'
  import { getLaunchOptions } from '$lib/features/sessions/harnesses/state.svelte'
  import { startShell } from '$lib/features/shells/service.svelte'
  import { getShellLaunchOptions } from '$lib/features/shells/state.svelte'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'

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

  let open = $derived(
    getTabs().filter(
      (t): t is ChatTab | SessionTab | ShellTab =>
        (t.kind === 'chat' || t.kind === 'session' || t.kind === 'shell') && t.folderPath === folderPath
    )
  )
  let history = $derived(historyRows(folderPath))
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
</script>

<SidebarPanel title="Sessions">
  {#snippet headerActions()}
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

  <div class="flex h-full min-h-0 flex-col">
    {#if liveAgents > 1}
      <Alert.Root variant="warning" layout="banner">
        <AlertTriangle size={13} class="mt-0.5 shrink-0" />
        <span>{liveAgents} agents share this working tree</span>
      </Alert.Root>
    {/if}
    <div class="min-h-0 flex-1">
      <Resizable.PaneGroup direction="vertical">
        <Resizable.Pane defaultSize={50} minSize={15}>
          <div class="h-full overflow-y-auto">
            {@render openList()}
          </div>
        </Resizable.Pane>
        <Resizable.Handle />
        <Resizable.Pane defaultSize={50} minSize={15}>
          {#key folderPath}
            <SessionHistory {folderPath} rows={history} />
          {/key}
        </Resizable.Pane>
      </Resizable.PaneGroup>
    </div>
  </div>
</SidebarPanel>

{#snippet openList()}
  {#each open as session (session.id)}
    <div class="group/row relative">
      <SidebarRow
        variant="leaf"
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
          tooltip={session.kind === 'shell' ? 'Close Shell' : 'Close Session'}
          onclick={(e: MouseEvent) => {
            e.stopPropagation()
            void closeTabWithChecks(session.id)
          }}
        >
          <X size={11} />
        </IconButton>
      </div>
    </div>
  {:else}
    <SidebarRow variant="info" divider={false}>No open sessions</SidebarRow>
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
  {/each}
{/snippet}
