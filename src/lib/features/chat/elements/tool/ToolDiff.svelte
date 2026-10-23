<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import FileIcon from '$lib/icons/FileIcon.svelte'
  import { diffRows, patchRows } from '$lib/features/chat/lineDiff'
  import type { ToolStep } from '$lib/features/chat/turns'
  import { toProjectRelativePath } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'

  let {
    call,
    folderPath,
    onOpen,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    call: ToolStep
    folderPath: string
    onOpen: () => void
  } = $props()
  const args = $derived(
    call.arguments !== null && typeof call.arguments === 'object' ? (call.arguments as Record<string, unknown>) : {}
  )
  const path = $derived(typeof args.path === 'string' ? args.path : '')
  const details = $derived(
    call.result?.details !== null && typeof call.result?.details === 'object'
      ? (call.result.details as Record<string, unknown>)
      : {}
  )
  const d = $derived(
    call.name === 'edit' && typeof details.patch === 'string'
      ? patchRows(details.patch)
      : call.name === 'write' && typeof args.content === 'string'
        ? diffRows(null, args.content)
        : null
  )
</script>

{#if d}
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
          <div class="px-2 whitespace-pre text-subtle">{row.count ? `${row.count} unchanged lines` : 'Diff hunk'}</div>
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
      {#if d.rows.length > 400}<div class="px-2 text-subtle">{d.rows.length - 400} more lines</div>{/if}
      {#if !d.rows.length}<div class="px-2 text-subtle">No changes</div>{/if}
    </div>
  </div>
{/if}
