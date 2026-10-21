<script lang="ts">
  import { onDestroy, untrack } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import MarkdownRenderer from '$lib/components/markdown/MarkdownRenderer.svelte'
  import { splitMarkdownBlocks } from '$lib/components/markdown/markdownBlocks'
  import { cn } from '$lib/utils/cn'

  let {
    text,
    streaming,
    folderPath,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    text: string
    streaming: boolean
    folderPath?: string
  } = $props()

  let sampled = $state(untrack(() => text))
  let timer: ReturnType<typeof setTimeout> | undefined
  const shown = $derived(streaming ? sampled : text)
  const blocks = $derived(splitMarkdownBlocks(shown))

  $effect(() => {
    const next = text
    if (!streaming) {
      clearTimeout(timer)
      timer = undefined
      sampled = next
    } else if (timer === undefined) {
      // Keep the first deadline: resetting it on every delta would debounce the stream.
      timer = setTimeout(() => {
        sampled = text
        timer = undefined
      }, 100)
    }
  })

  onDestroy(() => clearTimeout(timer))
</script>

<div data-slot="message-response" class={cn('flex flex-col gap-3', className)} {...rest}>
  {#each blocks as block, i (i)}
    <MarkdownRenderer source={block} {folderPath} class="p-0" />
  {/each}
</div>
