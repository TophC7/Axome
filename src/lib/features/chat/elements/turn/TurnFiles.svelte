<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import type { ChatEntry } from '$lib/types/backend'
  import FileIcon from '$lib/icons/FileIcon.svelte'
  import { basename, splitRemotePath, toProjectRelativePath } from '$lib/utils/paths'
  import { cn } from '$lib/utils/cn'

  let {
    steps,
    folderPath,
    onOpenLocation,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    steps: ChatEntry[]
    folderPath: string
    onOpenLocation: (path: string, line?: number) => void
  } = $props()
  const paths = $derived.by(() => {
    const paths = new Set<string>()
    for (const entry of steps) {
      if (entry.kind !== 'tool') continue
      for (const part of entry.content) {
        if (part.type === 'diff') paths.add(part.path)
      }
      if (entry.tool_kind === 'edit') {
        for (const location of entry.locations) paths.add(location.path)
      }
    }
    return [...paths]
  })
</script>

{#if paths.length}
  <div data-slot="turn-files" class={cn('flex flex-wrap gap-1.5', className)} {...rest}>
    {#each paths as path (path)}
      {@const relative =
        toProjectRelativePath(folderPath, path) ??
        toProjectRelativePath(splitRemotePath(folderPath)?.path ?? folderPath, path) ??
        path}
      <Tooltip.Root>
        <Tooltip.Trigger>
          {#snippet child({ props })}
            <Button
              {...props}
              variant="outline"
              size="xs"
              class="max-w-full font-mono"
              onclick={() => onOpenLocation(path)}
            >
              <FileIcon filename={path} size={12} />
              <span class="truncate">{basename(path)}</span>
            </Button>
          {/snippet}
        </Tooltip.Trigger>
        <Tooltip.Content>{relative}</Tooltip.Content>
      </Tooltip.Root>
    {/each}
  </div>
{/if}
