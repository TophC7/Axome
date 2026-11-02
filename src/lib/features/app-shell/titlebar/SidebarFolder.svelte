<!--
  @component
  SidebarFolder — the sidebar's folder in the title bar cell above it: a plain
  folder icon for local paths, a remote glyph plus `host:` for remote ones.
  Clicking opens the folder browser there to switch folders.
-->

<script lang="ts">
  import { isBrowserOpen, toggleBrowser } from '$lib/features/browser/state.svelte'
  import { ChevronsLeftRightEllipsisIcon, Folder01Icon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { basename, parentPath, splitRemotePath } from '$lib/utils/paths'

  let { folderPath }: { folderPath: string } = $props()

  let remote = $derived(splitRemotePath(folderPath))
  let localPath = $derived(remote?.path ?? folderPath)
  let name = $derived(basename(localPath) || '/')
  let parent = $derived(parentPath(localPath) || '/')
</script>

<button
  type="button"
  class="group flex h-full w-full cursor-pointer items-center gap-2 px-2.5 text-left text-sm transition-colors hover:bg-raised focus-visible:shadow-focus-ring focus-visible:outline-none"
  aria-label="Switch folder (current: {folderPath})"
  aria-haspopup="dialog"
  aria-expanded={isBrowserOpen()}
  title={folderPath}
  onclick={() => toggleBrowser({ path: folderPath })}
>
  <span class="shrink-0 text-bright uppercase">{name}</span>
  <span class="min-w-0 truncate font-mono text-xs text-subtle">
    {#if remote}<span class="text-muted">{remote.server}:</span>{/if}{parent}
  </span>
  {#if remote}
    <Icon icon={ChevronsLeftRightEllipsisIcon} size={12} class="ml-auto shrink-0 text-subtle group-hover:text-fg" />
  {:else}
    <Icon icon={Folder01Icon} size={12} class="ml-auto shrink-0 text-subtle group-hover:text-fg" />
  {/if}
</button>
