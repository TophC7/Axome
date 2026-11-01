<script lang="ts">
  import type { ChatCommand } from '$lib/types/backend'
  import { menuContentVariants, menuItemClass } from '$lib/components/ui/menu-recipe'
  import { cn } from '$lib/utils/cn'
  import { logicalKey } from '$lib/utils/keyboardEvent'

  let {
    text = $bindable(''),
    commands,
    disabled = false
  }: {
    text?: string
    commands: ChatCommand[]
    disabled?: boolean
  } = $props()
  const id = $props.id()
  let dismissed = $state<string | null>(null)
  let selection = $state({ text: '', index: 0 })
  const query = $derived(/^\/(\S*)$/.exec(text)?.[1])
  const filtered = $derived(query === undefined ? [] : commands.filter((command) => command.name.startsWith(query)))
  const open = $derived(!disabled && query !== undefined && filtered.length > 0 && dismissed !== text)
  const active = $derived(selection.text === text ? Math.min(selection.index, filtered.length - 1) : 0)

  function select(index: number) {
    const command = filtered[index]
    if (command) text = '/' + command.name + ' '
  }
  export function activeDescendant(): string | undefined {
    return open ? `${id}-${active}` : undefined
  }
  export function listboxId(): string | undefined {
    return open ? id : undefined
  }
  export function handleKeydown(event: KeyboardEvent): boolean {
    const key = logicalKey(event)
    if (
      event.isComposing ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey ||
      (key === 'Tab' && event.shiftKey) ||
      !open ||
      !['ArrowUp', 'ArrowDown', 'Tab', 'Enter', 'Escape'].includes(key)
    )
      return false
    event.preventDefault()
    if (key === 'Escape') dismissed = text
    else if (key === 'Enter' || key === 'Tab') select(active)
    else selection = { text, index: (active + (key === 'ArrowDown' ? 1 : -1) + filtered.length) % filtered.length }
    return true
  }
</script>

{#if open}
  <div
    data-slot="prompt-input-slash-menu"
    {id}
    role="listbox"
    aria-label="Commands"
    tabindex="-1"
    class={cn(menuContentVariants(), 'absolute bottom-full left-0 mb-1 max-h-64 w-full')}
  >
    {#each filtered as command, index (command.name)}
      <button
        type="button"
        id={`${id}-${index}`}
        role="option"
        aria-selected={index === active}
        tabindex="-1"
        data-highlighted={index === active ? '' : undefined}
        class={cn(menuItemClass, 'w-full text-left')}
        onpointerdown={(event) => event.preventDefault()}
        onclick={() => select(index)}
      >
        <span class="font-mono text-fg">/{command.name}</span>
        <span class="truncate text-subtle">— {command.description}</span>
      </button>
    {/each}
  </div>
{/if}
