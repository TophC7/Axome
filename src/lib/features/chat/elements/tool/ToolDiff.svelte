<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import FileIcon from '$lib/icons/FileIcon.svelte'
  import { diffRows } from '$lib/features/chat/lineDiff'
  import { toProjectRelativePath } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'

  let {
    path,
    oldText,
    newText,
    folderPath,
    onOpen,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    path: string
    oldText: string | null
    newText: string
    folderPath: string
    onOpen: () => void
  } = $props()
  const d = $derived(diffRows(oldText, newText))
</script>

<div data-slot="tool-diff" class={cn('flex min-w-0 flex-col gap-1.5', className)} {...rest}>
  <div class="flex items-center gap-1.5 text-xs">
    <FileIcon filename={path} size={12} />
    <span class="truncate font-mono text-fg">{toProjectRelativePath(folderPath, path) ?? path}</span>
    <span class="font-mono text-success">+{d.added}</span>
    <span class="font-mono text-danger">−{d.removed}</span>
    <Button variant="ghost" size="xs" class="ms-auto" onclick={onOpen}>Open</Button>
  </div>
  <div class="max-h-80 overflow-auto rounded-md border border-edge bg-surface font-mono text-xs">
    {#each d.rows.slice(0, 400) as row}
      {#if row.kind === 'gap'}
        <div class="px-2 whitespace-pre text-subtle">⋯ {row.count} unchanged lines</div>
      {:else}
        <div
          class={cn(
            'flex whitespace-pre',
            row.kind === 'add'
              ? 'bg-success-bg text-success-bright'
              : row.kind === 'del'
                ? 'bg-danger-bg text-danger-bright'
                : 'text-muted'
          )}
        >
          <span class="w-[1ch] shrink-0" aria-hidden="true"
            >{row.kind === 'add' ? '+' : row.kind === 'del' ? '−' : ' '}</span
          ><span>{row.text}</span>
        </div>
      {/if}
    {/each}
    {#if d.rows.length > 400}<div class="px-2 text-subtle">⋯ {d.rows.length - 400} more lines</div>{/if}
    {#if !d.rows.length}<div class="px-2 text-subtle">No changes</div>{/if}
  </div>
</div>
