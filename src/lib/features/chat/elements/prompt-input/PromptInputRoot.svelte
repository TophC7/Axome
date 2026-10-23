<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import type { ChatCommand } from '$lib/types/backend'
  import type { ChatDraftImage } from './types'
  import { Textarea } from '$lib/components/ui/input'
  import { cn } from '$lib/utils/cn'
  import Attachments from './PromptInputAttachments.svelte'
  import SlashMenu from './PromptInputSlashMenu.svelte'

  let {
    text = $bindable(''),
    images,
    acceptsImages,
    disabled = false,
    placeholder,
    commands,
    onAddImages,
    onRemoveImage,
    onSubmit,
    toolbar,
    class: className,
    ...rest
  }: Omit<HTMLAttributes<HTMLDivElement>, 'onpaste' | 'ondrop' | 'ondragover'> & {
    text?: string
    images: ChatDraftImage[]
    acceptsImages: boolean
    disabled?: boolean
    placeholder: string
    commands: ChatCommand[]
    onAddImages: (files: File[]) => void
    onRemoveImage: (id: string) => void
    onSubmit: () => void
    toolbar?: Snippet
  } = $props()
  let ref = $state<HTMLTextAreaElement | null>(null)
  let menu = $state<SlashMenu | undefined>()
  $effect(() => {
    void text
    if (!ref) return
    ref.style.height = 'auto'
    ref.style.height = `${Math.min(ref.scrollHeight, 240)}px`
  })
  function keydown(event: KeyboardEvent) {
    if (event.isComposing || disabled || menu?.handleKeydown(event)) return
    if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      onSubmit()
    }
  }
  function paste(event: ClipboardEvent) {
    const files = Array.from(event.clipboardData?.files ?? []).filter((file) => file.type.startsWith('image/'))
    if (!files.length) return
    event.preventDefault()
    if (!disabled) onAddImages(files)
  }
  function drop(event: DragEvent) {
    event.preventDefault()
    if (disabled) return
    const files = Array.from(event.dataTransfer?.files ?? []).filter((file) => file.type.startsWith('image/'))
    if (files.length) onAddImages(files)
  }
</script>

<div
  data-slot="prompt-input-root"
  data-accepts-images={acceptsImages}
  role="group"
  aria-label="Message composer"
  class={cn('border-t border-edge bg-ground px-4 pt-2 pb-3', className)}
  ondragover={(event) => {
    if (event.dataTransfer?.types.includes('Files')) event.preventDefault()
  }}
  ondrop={drop}
  {...rest}
>
  <div
    class="relative mx-auto max-w-3xl rounded-lg border border-edge bg-surface transition-colors focus-within:border-accent"
  >
    <SlashMenu bind:this={menu} bind:text {commands} {disabled} />
    {#if images.length}<Attachments {images} {disabled} onRemove={onRemoveImage} />{/if}
    <Textarea
      bind:ref
      bind:value={text}
      rows={1}
      {disabled}
      {placeholder}
      aria-label={placeholder}
      aria-autocomplete="list"
      aria-controls={menu?.listboxId()}
      aria-activedescendant={menu?.activeDescendant()}
      class="max-h-60 min-h-10 overflow-y-auto border-0 bg-transparent px-3 py-2 focus:border-0"
      onkeydown={keydown}
      onpaste={paste}
    />
    {#if toolbar}{@render toolbar()}{/if}
  </div>
</div>
