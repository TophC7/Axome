<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import type { ChatCommand } from '$lib/types/backend'
  import { Textarea } from '$lib/components/ui/input'
  import { cn } from '$lib/utils/cn'
  import SlashMenu from './PromptInputSlashMenu.svelte'

  let {
    text = $bindable(''),
    disabled = false,
    placeholder,
    commands,
    onSubmit,
    onAltSubmit,
    toolbar,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    text?: string
    disabled?: boolean
    placeholder: string
    commands: ChatCommand[]
    onSubmit: () => void
    onAltSubmit: () => void
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
    if (event.isComposing || disabled) return
    if (event.key === 'Enter' && event.altKey && !event.shiftKey) {
      event.preventDefault()
      onAltSubmit()
    } else if (menu?.handleKeydown(event)) return
    else if (event.key === 'Enter' && !event.shiftKey) {
      event.preventDefault()
      onSubmit()
    }
  }
</script>

<div
  data-slot="prompt-input-root"
  role="group"
  aria-label="Message composer"
  class={cn('border-t border-edge bg-ground px-4 pt-2 pb-3', className)}
  {...rest}
>
  <div
    class="relative mx-auto max-w-3xl rounded-lg border border-edge bg-surface transition-colors focus-within:border-accent"
  >
    <SlashMenu bind:this={menu} bind:text {commands} {disabled} />
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
    />
    {#if toolbar}{@render toolbar()}{/if}
  </div>
</div>
