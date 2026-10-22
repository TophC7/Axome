<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import * as Dialog from '$lib/components/ui/dialog'
  import { IconButton } from '$lib/components/ui/button'
  import { CopyIcon, X } from '$lib/icons/lucideExports'
  import type { ChatContent } from '$lib/types/backend'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { cn } from '$lib/utils/cn'

  let { content, class: className, ...rest }: HTMLAttributes<HTMLDivElement> & { content: ChatContent[] } = $props()

  type Image = Extract<ChatContent, { type: 'image' }>
  type Part = { type: 'text'; text: string } | { type: 'images'; images: Image[] }
  const parts = $derived.by(() => {
    const result: Part[] = []
    for (const part of content) {
      const previous = result.at(-1)
      if (part.type === 'text') {
        if (previous?.type === 'text') previous.text += part.text
        else result.push({ type: 'text', text: part.text })
      } else if (previous?.type === 'images') {
        previous.images.push(part)
      } else {
        result.push({ type: 'images', images: [part] })
      }
    }
    return result
  })
  const text = $derived(content.flatMap((part) => (part.type === 'text' ? [part.text] : [])).join('\n'))
  let selected = $state<Image | null>(null)
</script>

<div data-slot="message-user" class={cn('group', className)} {...rest}>
  <div
    class="ml-auto max-w-[85%] rounded-lg border border-edge bg-raised px-3 py-2 text-base break-words whitespace-pre-wrap text-fg"
  >
    {#each parts as part}
      {#if part.type === 'text'}
        {part.text}
      {:else}
        <div class="flex flex-wrap gap-1.5">
          {#each part.images as image}
            <button
              type="button"
              aria-label="Open Attached Image"
              class="size-16 overflow-hidden rounded-md border border-edge focus-visible:shadow-focus-ring focus-visible:outline-none"
              onclick={() => (selected = image)}
            >
              <img
                src={`data:${image.mime_type};base64,${image.data}`}
                alt="Attachment"
                class="size-full object-cover"
              />
            </button>
          {/each}
        </div>
      {/if}
    {/each}
  </div>
  {#if text}
    <div
      class="duration-fast flex h-6 items-center justify-end opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100"
    >
      <IconButton tooltip="Copy Message" onclick={() => copyToClipboard(text, 'Copy message failed')}>
        <CopyIcon size={12} />
      </IconButton>
    </div>
  {/if}
</div>

<Dialog.Root
  open={selected !== null}
  onOpenChange={(open) => {
    if (!open) selected = null
  }}
>
  <Dialog.Content class="max-w-3xl" onModalClose={() => (selected = null)}>
    <div class="mb-3 flex items-center justify-between gap-2">
      <Dialog.Title>Attached Image</Dialog.Title>
      <IconButton tooltip="Close Image" onclick={() => (selected = null)}><X size={14} /></IconButton>
    </div>
    <Dialog.Description class="sr-only">Full-size attached image</Dialog.Description>
    {#if selected}
      <img
        src={`data:${selected.mime_type};base64,${selected.data}`}
        alt="Attachment preview"
        class="mx-auto max-h-[80vh] max-w-full"
      />
    {/if}
  </Dialog.Content>
</Dialog.Root>
