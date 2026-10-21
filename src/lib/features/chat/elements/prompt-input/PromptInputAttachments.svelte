<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import type { ChatDraftImage } from '$lib/features/chat/controller.svelte'
  import { IconButton } from '$lib/components/ui/button'
  import { X } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'

  let {
    images,
    onRemove,
    disabled = false,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    images: ChatDraftImage[]
    onRemove: (id: string) => void
    disabled?: boolean
  } = $props()
</script>

<div data-slot="prompt-input-attachments" class={cn('flex flex-wrap gap-1.5 px-2 pt-2', className)} {...rest}>
  {#each images as image (image.id)}
    <div class="relative size-12">
      <img
        src={`data:${image.mimeType};base64,${image.data}`}
        alt={image.name}
        class="size-12 rounded-md border border-edge object-cover"
      />
      <IconButton
        tooltip="Remove Image"
        class="absolute -top-1 -right-1 bg-raised"
        {disabled}
        onclick={() => onRemove(image.id)}><X size={10} /></IconButton
      >
    </div>
  {/each}
</div>
