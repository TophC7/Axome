<!--
  @component
  SessionsSidebar — the folder's sessions (agents and terminals) as
  shortcuts. Open ones focus their tab; recently closed ones (this launch)
  reopen as a tab, resuming the provider conversation when a resume token
  was captured, otherwise starting fresh. The header's + starts a new one.
-->

<script lang="ts">
  import SidebarPanel from '$lib/features/app-shell/sidebar/SidebarPanel.svelte'
  import { IconButton, iconButtonVariants } from '$lib/components/ui/button'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { SidebarRow } from '$lib/components/ui/sidebar-row'
  import { AlertTriangle, ChevronDown, ChevronRight, HistoryIcon, Plus, X } from '$lib/icons/lucideExports'
  import type { SessionStatus, SessionTab } from '$lib/features/workbench/model'
  import {
    getActiveTabId,
    getClosedSessions,
    getTabs,
    reopenClosedTab,
    setActiveTab
  } from '$lib/features/workbench/state.svelte'
  import { closeTabWithChecks } from '$lib/features/workbench/tabActions.svelte'
  import { countLiveAgents, startSession } from '$lib/features/sessions/service.svelte'
  import { allProviders, directOptions, providerById } from '$lib/features/sessions/providers/catalog'
  import { getConnectedProviders } from '$lib/features/sessions/providers/state.svelte'
  import ProviderIcon from '$lib/features/sessions/providers/ProviderIcon.svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { notify } from '$lib/features/notifications/state.svelte'
  import { getErrorMessage } from '$lib/utils/client-error'

  let { folderPath }: { folderPath: string } = $props()

  const SESSION_TONE: Record<SessionStatus, StatusTone> = {
    running: 'working',
    starting: 'waiting',
    failed: 'danger',
    exited: 'idle',
    dormant: 'idle'
  }

  let open = $derived(getTabs().filter((t): t is SessionTab => t.kind === 'session' && t.folderPath === folderPath))
  let closed = $derived(getClosedSessions(folderPath))
  let activeTabId = $derived(getActiveTabId())
  let liveAgents = $derived(countLiveAgents(folderPath))
  let launchers = $derived.by(() => {
    const connected = new Set(getConnectedProviders(folderPath).map((p) => p.id as string))
    return [...allProviders, ...directOptions].filter((p) => connected.has(p.id))
  })

  let openCollapsed = $state(false)
  let closedCollapsed = $state(false)
</script>

<SidebarPanel title="Sessions">
  {#snippet headerActions()}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger aria-label="New session" class={iconButtonVariants()}>
        <Plus size={12} />
      </DropdownMenu.Trigger>
      <DropdownMenu.Content align="end">
        {#each launchers as provider (provider.id)}
          <DropdownMenu.Item onSelect={() => startSession(folderPath, provider.id)}>
            <ProviderIcon icon={provider.icon} size={14} />
            <span>{provider.label}</span>
          </DropdownMenu.Item>
        {:else}
          <DropdownMenu.Item disabled>No agent CLIs detected</DropdownMenu.Item>
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
    {#if open.length === 0 && closed.length === 0}
      <SidebarRow variant="info" divider={false}>No sessions in this folder yet</SidebarRow>
      {#each launchers as provider (provider.id)}
        <SidebarRow variant="action" divider={false} onclick={() => startSession(folderPath, provider.id)}>
          <ProviderIcon icon={provider.icon} size={12} />
          <span class="min-w-0 flex-1 truncate">Start {provider.label}</span>
          <Plus size={12} class="shrink-0" />
        </SidebarRow>
      {/each}
    {/if}

    {#if open.length > 0}
      {@render section('Open', open.length, openCollapsed, () => (openCollapsed = !openCollapsed))}
      {#if !openCollapsed}
        <div class="border-t border-edge/30 bg-surface py-1">
          {#each open as session (session.id)}
            {@const meta = providerById[session.providerId]}
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
                {#if meta}<ProviderIcon icon={meta.icon} size={12} />{/if}
                <span class="min-w-0 flex-1 truncate {session.id === activeTabId ? 'text-bright' : 'text-fg'}">
                  {session.title}
                </span>
                <StatusDot tone={SESSION_TONE[session.status]} size="sm" />
              </SidebarRow>
              <div
                class="absolute inset-y-0 right-1 flex items-center opacity-0 group-hover/row:opacity-100 group-has-[:focus-visible]/row:opacity-100"
              >
                <IconButton
                  tooltip="Close session"
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

    {#if closed.length > 0}
      {@render section('Recently closed', closed.length, closedCollapsed, () => (closedCollapsed = !closedCollapsed))}
      {#if !closedCollapsed}
        <div class="border-t border-edge/30 bg-surface py-1">
          {#each closed as snapshot (snapshot)}
            {@const meta = providerById[snapshot.providerId]}
            <SidebarRow
              variant="leaf"
              on="surface"
              divider={false}
              title={snapshot.resumeToken ? 'Reopen and resume this conversation' : 'Reopen (starts fresh)'}
              onclick={() =>
                void reopenClosedTab(snapshot).catch((error) =>
                  notify.error('Reopen session failed', getErrorMessage(error))
                )}
            >
              {#if meta}<ProviderIcon icon={meta.icon} size={12} class="opacity-60" />{/if}
              <span class="min-w-0 flex-1 truncate text-muted">{snapshot.title}</span>
              {#if snapshot.resumeToken}
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
