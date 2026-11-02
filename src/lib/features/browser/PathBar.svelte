<script lang="ts">
  import { backend } from '$lib/api/backend'
  import { Button } from '$lib/components/ui/button'
  import { HardDriveIcon, Home01Icon, MonitorIcon, ServerIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import type { PathRoot } from '$lib/types/backend'
  import { isEqualOrParent, normalizeAbsolutePath, splitRemotePath } from '$lib/utils/paths'
  import { localHostLabel } from './state.svelte'

  let {
    path,
    onnavigate,
    onhosts,
    hostPinned = false
  }: {
    path: string
    onnavigate: (path: string) => void
    onhosts: () => void
    hostPinned?: boolean
  } = $props()

  let remote = $derived(splitRemotePath(path))
  let root = $derived(remote ? `axome://${remote.server}/` : '/')
  let pathRoot = $state<PathRoot | null>(null)
  let anchor = $derived(pathRoot && pathRoot.path !== root && isEqualOrParent(pathRoot.path, path) ? pathRoot : null)
  let crumbs = $derived.by(() => {
    let ancestor = (anchor?.path ?? root).replace(/\/$/, '')
    return normalizeAbsolutePath(path)
      .slice((anchor?.path ?? root).length)
      .split('/')
      .filter(Boolean)
      .map((label) => {
        ancestor += `/${label}`
        return { label, path: ancestor }
      })
  })

  $effect(() => {
    const target = path
    let cancelled = false
    backend.folders
      .pathRoot(target)
      .then((next) => {
        if (!cancelled) pathRoot = next
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  })

  function keepEndVisible(element: HTMLElement): void {
    void crumbs
    element.scrollLeft = element.scrollWidth
  }
</script>

<nav
  aria-label="Folder location"
  class="flex min-w-0 flex-1 scrollbar-none items-center gap-0.5 overflow-x-auto"
  {@attach keepEndVisible}
>
  <Button
    variant="ghost"
    size="xs"
    class="shrink-0"
    disabled={hostPinned}
    title={hostPinned ? 'Selection stays on this server' : 'Choose Server'}
    onclick={onhosts}
  >
    {#if remote}<Icon icon={ServerIcon} size={12} />{:else}<Icon icon={MonitorIcon} size={12} />{/if}
    {remote?.server ?? localHostLabel()}
  </Button>
  <Button variant="ghost" size="xs" class="shrink-0 font-mono" title={root} onclick={() => onnavigate(root)}>/</Button>
  {#if anchor}
    <Button variant="ghost" size="xs" class="shrink-0" title={anchor.path} onclick={() => onnavigate(anchor.path)}>
      {#if anchor.kind === 'home'}<Icon icon={Home01Icon} size={12} />{:else}<Icon
          icon={HardDriveIcon}
          size={12}
        />{/if}
      {anchor.label}
    </Button>
  {/if}
  {#each crumbs as crumb, index (crumb.path)}
    {#if index > 0 || anchor}<span class="text-subtle" aria-hidden="true">/</span>{/if}
    {#if index === crumbs.length - 1}
      <span class="shrink-0 px-1.5 font-mono text-xs text-bright" aria-current="location">{crumb.label}</span>
    {:else}
      <Button
        variant="ghost"
        size="xs"
        class="shrink-0 font-mono"
        title={crumb.path}
        onclick={() => onnavigate(crumb.path)}>{crumb.label}</Button
      >
    {/if}
  {/each}
</nav>
