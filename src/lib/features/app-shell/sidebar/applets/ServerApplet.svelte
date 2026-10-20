<!--
  @component
  ServerApplet — the page's own server: connection state, recent folders
  here, browse it. Web counterpart of RemotesApplet.
-->

<script lang="ts">
  import RailApplet, { appletHeadingClass, appletRowClass } from './RailApplet.svelte'
  import type { ToneStatus } from '$lib/components/ui/status-dot'
  import { FolderIcon, SearchIcon, ServerIcon } from '$lib/icons/lucideExports'
  import { getHostConnection } from '$lib/api/hostConnection.svelte'
  import type { WebConnectionState } from '$lib/api/transport.web'
  import { goToFolder, localHostLabel, openBrowser } from '$lib/features/browser/state.svelte'
  import { getRecentFolders } from '$lib/features/folders/state.svelte'
  import { basename, parentPath } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'

  const SHOWN_RECENTS = 5
  const CONNECTION: Record<WebConnectionState, ToneStatus> = {
    connecting: { tone: 'waiting', text: 'Connecting…' },
    connected: { tone: 'success', text: 'Connected' },
    reconnecting: { tone: 'waiting', text: 'Reconnecting…' },
    busy: { tone: 'waiting', text: 'Busy' },
    revoked: { tone: 'danger', text: 'Access revoked' },
    closed: { tone: 'idle', text: 'Closed' },
    error: { tone: 'danger', text: 'Connection error' }
  }

  let open = $state(false)
  let status = $derived(CONNECTION[getHostConnection() ?? 'connecting'])
  let recents = $derived(getRecentFolders().slice(0, SHOWN_RECENTS))

  function go(action: () => Promise<void> | void) {
    open = false
    void action()
  }
</script>

<span class="sr-only" role="status" aria-live="polite">Server {status.text}</span>

<RailApplet bind:open label="Server" {status}>
  {#snippet icon()}
    <ServerIcon size={16} />
  {/snippet}

  <p class="px-2 pb-1 text-xs text-subtle">Terminal, LSP, and file streams reconnect on their own.</p>

  {#if recents.length}
    <div class={appletHeadingClass}>Recent</div>
    {#each recents as { path } (path)}
      <button type="button" class={appletRowClass} title={path} onclick={() => go(() => goToFolder(path))}>
        <FolderIcon size={12} class="shrink-0 text-muted" />
        <span class="shrink-0">{basename(path) || '/'}</span>
        <span class="min-w-0 truncate font-mono text-xs text-subtle">{parentPath(path)}</span>
      </button>
    {/each}
  {/if}

  <button
    type="button"
    class={cn(appletRowClass, 'text-muted')}
    onclick={() => go(() => openBrowser({ server: null }))}
  >
    <SearchIcon size={12} class="shrink-0" />
    <span>Browse {localHostLabel()}…</span>
  </button>
</RailApplet>
