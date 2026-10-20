<!--
  @component
  SidebarFooter — the sidebar's folder, as one flat row: a plain folder icon
  for local paths, a remote glyph plus `host:` for remote ones. Clicking
  opens the folder browser there to switch folders.
-->

<script lang="ts">
  import { isBrowserOpen, toggleBrowser } from '$lib/features/browser/state.svelte'
  import { ChevronsLeftRightEllipsisIcon, ChevronsUpDown, FolderIcon } from '$lib/icons/lucideExports'
  import { basename, parentPath, splitRemotePath } from '$lib/utils/paths'

  let { folderPath }: { folderPath: string } = $props()

  let remote = $derived(splitRemotePath(folderPath))
  let localPath = $derived(remote?.path ?? folderPath)
  let name = $derived(basename(localPath) || '/')
  let parent = $derived(parentPath(localPath) || '/')
</script>

<button
  type="button"
  class="group flex h-8 w-full shrink-0 cursor-pointer items-center gap-2 border-t border-edge bg-ground px-2.5 text-left text-sm transition-colors hover:bg-surface focus-visible:shadow-focus-ring focus-visible:outline-none"
  data-browser-toggle="true"
  aria-label="Switch folder (current: {folderPath})"
  aria-haspopup="dialog"
  aria-expanded={isBrowserOpen()}
  title={folderPath}
  onclick={() => toggleBrowser({ path: folderPath })}
>
  {#if remote}
    <ChevronsLeftRightEllipsisIcon size={12} class="shrink-0 text-muted" />
  {:else}
    <FolderIcon size={12} class="shrink-0 text-muted" />
  {/if}
  <span class="shrink-0 text-bright">{name}</span>
  <span class="min-w-0 truncate font-mono text-xs text-subtle">
    {#if remote}<span class="text-muted">{remote.server}:</span>{/if}{parent}
  </span>
  <ChevronsUpDown size={12} class="ml-auto shrink-0 text-subtle group-hover:text-fg" />
</button>
