<!--
  @component
  RemotesApplet — desktop rail applet for paired servers: each server's
  state, its workbenches and the folders recently opened on it, plus
  browsing it. Shown whenever servers are paired, whether or not this window
  is on one; the status tracks the current folder's server when it is remote.
-->

<script lang="ts">
  import { onMount } from 'svelte'
  import RailApplet, { appletRowClass } from './RailApplet.svelte'
  import { Button } from '$lib/components/ui/button'
  import { StatusDot, type ToneStatus } from '$lib/components/ui/status-dot'
  import { FolderIcon, Layers, SearchIcon, ServerIcon } from '$lib/icons/lucideExports'
  import {
    getServers,
    getWorkbenchSections,
    watchPlaces,
    watchServers,
    workbenchDetail,
    workbenchTitle,
    type ServerState
  } from '$lib/features/browser/places.svelte'
  import { goToFolder, goToWorkbench, openBrowser } from '$lib/features/browser/state.svelte'
  import { getRecentFolders } from '$lib/features/folders/state.svelte'
  import { remoteTone } from '$lib/features/remotes/remoteTone'
  import { openRemoteManager } from '$lib/features/remotes/state.svelte'
  import { basename, parentPath, splitRemotePath } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'

  let { folderPath }: { folderPath: string | null } = $props()

  const RECENTS_PER_SERVER = 3
  const STATE_LABEL: Record<ServerState, string> = {
    connected: 'Connected',
    reconnecting: 'Reconnecting…',
    disconnected: 'Offline',
    error: 'Error',
    checking: 'Checking…'
  }

  let open = $state(false)
  let servers = $derived(getServers())
  let here = $derived(folderPath ? (splitRemotePath(folderPath)?.server ?? null) : null)
  let hereState = $derived(servers.find((s) => s.name === here)?.state ?? null)
  let connectedCount = $derived(servers.filter((s) => s.state === 'connected').length)
  let status = $derived<ToneStatus>(
    here && hereState
      ? { tone: remoteTone(hereState), text: `${here}: ${STATE_LABEL[hereState]}` }
      : { tone: 'idle', text: `${connectedCount} of ${servers.length} connected` }
  )

  let workbenchesBy = $derived(new Map(getWorkbenchSections().map((s) => [s.server, s.workbenches])))
  let recentsBy = $derived.by(() => {
    const byServer = new Map<string, { path: string; remotePath: string }[]>()
    for (const { path } of getRecentFolders()) {
      const remote = splitRemotePath(path)
      if (!remote) continue
      const list = byServer.get(remote.server) ?? []
      if (list.length < RECENTS_PER_SERVER) list.push({ path, remotePath: remote.path })
      byServer.set(remote.server, list)
    }
    return byServer
  })

  onMount(watchServers)

  $effect(() => {
    if (open) return watchPlaces()
  })

  function go(action: () => Promise<void> | void) {
    open = false
    void action()
  }
</script>

{#if servers.length > 0}
  <RailApplet bind:open label="Remote servers" {status}>
    {#snippet icon()}
      <ServerIcon size={16} />
    {/snippet}

    <div class="flex max-h-96 flex-col gap-2 overflow-y-auto">
      {#each servers as server (server.name)}
        {@const workbenches = workbenchesBy.get(server.name) ?? []}
        {@const recents = recentsBy.get(server.name) ?? []}
        <section>
          <div class="flex h-6 items-center gap-2 px-2" title={server.lastError ?? undefined}>
            <StatusDot tone={remoteTone(server.state)} size="sm" />
            <span class="min-w-0 flex-1 truncate font-mono text-sm {server.name === here ? 'text-bright' : 'text-fg'}">
              {server.name}
            </span>
            <span class="shrink-0 text-xs {server.state === 'error' ? 'text-danger' : 'text-subtle'}">
              {STATE_LABEL[server.state]}
            </span>
          </div>

          {#each workbenches as workbench (workbench.id)}
            {@const detail = workbenchDetail(workbench)}
            <button
              type="button"
              class={appletRowClass}
              onclick={() => go(() => goToWorkbench(workbench, server.name))}
            >
              <Layers size={12} class="shrink-0 text-muted" />
              <span class="min-w-0 flex-1 truncate">{workbenchTitle(workbench)}</span>
              {#if detail}<span class="shrink-0 text-xs text-subtle">{detail}</span>{/if}
            </button>
          {/each}

          {#each recents as recent (recent.path)}
            <button
              type="button"
              class={appletRowClass}
              title={recent.path}
              onclick={() => go(() => goToFolder(recent.path))}
            >
              <FolderIcon size={12} class="shrink-0 text-muted" />
              <span class="shrink-0">{basename(recent.remotePath) || '/'}</span>
              <span class="min-w-0 truncate font-mono text-xs text-subtle">{parentPath(recent.remotePath)}</span>
            </button>
          {/each}

          <button
            type="button"
            class={cn(appletRowClass, 'text-muted')}
            onclick={() => go(() => openBrowser({ server: server.name }))}
          >
            <SearchIcon size={12} class="shrink-0" />
            <span>Browse {server.name}…</span>
          </button>
        </section>
      {/each}
    </div>

    {#snippet actions()}
      <Button variant="ghost" size="xs" onclick={() => go(openRemoteManager)}>Manage servers</Button>
    {/snippet}
  </RailApplet>
{/if}
