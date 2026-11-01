<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { ChatCommand } from '$lib/types/backend'
  import { logicalKey } from '$lib/utils/keyboardEvent'
  import SlashMenu from './PromptInputSlashMenu.svelte'

  let {
    text = $bindable(''),
    disabled = false,
    placeholder,
    commands,
    onSubmit,
    onAltSubmit,
    start,
    end
  }: {
    text?: string
    disabled?: boolean
    placeholder: string
    commands: ChatCommand[]
    onSubmit: () => void
    onAltSubmit: () => void
    start?: Snippet
    end?: Snippet
  } = $props()
  let ref = $state<HTMLTextAreaElement | null>(null)
  let menu = $state<SlashMenu | undefined>()

  export function focus(insert?: string): void {
    if (!ref || disabled) return
    ref.focus()
    if (insert) {
      ref.setRangeText(insert, ref.selectionStart, ref.selectionEnd, 'end')
      text = ref.value
    }
  }

  $effect(() => {
    void text
    if (!ref) return
    ref.style.height = 'auto'
    ref.style.height = `${ref.scrollHeight}px`
  })

  function keydown(event: KeyboardEvent) {
    if (event.isComposing || disabled || event.defaultPrevented) return
    const key = logicalKey(event)
    if (key === 'Enter' && event.altKey && !event.shiftKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault()
      onAltSubmit()
    } else if (menu?.handleKeydown(event)) return
    else if (key === 'Enter' && !event.shiftKey && !event.altKey && !event.ctrlKey && !event.metaKey) {
      event.preventDefault()
      onSubmit()
    }
  }
</script>

<div
  data-slot="prompt-input-root"
  role="group"
  aria-label="Message composer"
  class="relative flex flex-col rounded-lg border border-edge bg-surface transition-colors focus-within:border-accent"
>
  <SlashMenu bind:this={menu} bind:text {commands} {disabled} />
  <textarea
    bind:this={ref}
    bind:value={text}
    rows={1}
    {disabled}
    {placeholder}
    aria-label={placeholder}
    aria-autocomplete="list"
    aria-controls={menu?.listboxId()}
    aria-activedescendant={menu?.activeDescendant()}
    class="max-h-60 min-h-10 w-full resize-none overflow-y-auto bg-transparent px-3 pt-2.5 pb-1 text-base leading-normal text-fg outline-none placeholder:text-subtle disabled:opacity-50"
    onkeydown={keydown}
  ></textarea>
  <div class="flex items-center gap-1 px-1.5 pb-1.5">
    {#if start}{@render start()}{/if}
    <div class="ms-auto flex min-w-0 items-center gap-1">
      {#if end}{@render end()}{/if}
    </div>
  </div>
</div>
