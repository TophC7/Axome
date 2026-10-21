<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import * as Popover from '$lib/components/ui/popover'
  import * as Command from '$lib/components/ui/command'
  import { BrainIcon, Check, ChevronDown, SparklesIcon } from '$lib/icons/lucideExports'
  import type { ChatConfigChoice, ChatConfigOption } from '$lib/types/backend'
  import { cn } from '$lib/utils/cn'

  let {
    option,
    disabled = false,
    onSelect,
    class: className,
    ...rest
  }: HTMLButtonAttributes & {
    option: ChatConfigOption
    disabled?: boolean
    onSelect: (value: string) => void
  } = $props()

  let open = $state(false)
  const selection = $derived(option.kind.type === 'select' ? option.kind : null)
  const choices = $derived(selection?.choices ?? [])
  const currentName = $derived(
    choices.find((choice) => choice.value === selection?.current_value)?.name ?? selection?.current_value ?? ''
  )
  const groups = $derived.by(() => {
    const result = new Map<string | null, ChatConfigChoice[]>()
    for (const choice of choices) {
      const group = result.get(choice.group)
      if (group) group.push(choice)
      else result.set(choice.group, [choice])
    }
    return Array.from(result, ([heading, choices]) => ({ heading: heading ?? undefined, choices }))
  })

  function select(value: string) {
    onSelect(value)
    open = false
  }
</script>

{#snippet trigger(triggerProps: Record<string, unknown>)}
  <Tooltip.Root>
    <Tooltip.Trigger {...triggerProps} {disabled}>
      {#snippet child({ props }: { props: Record<string, unknown> })}
        <Button
          {...props}
          data-slot="config-picker"
          variant="ghost"
          size="xs"
          aria-label={option.name}
          {disabled}
          class={cn('max-w-48', className)}
          {...rest}
        >
          {#if option.category === 'model'}
            <SparklesIcon size={12} />
          {:else if option.category === 'thought_level'}
            <BrainIcon size={12} />
          {/if}
          <span class="truncate">{currentName}</span>
          <ChevronDown size={12} />
        </Button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>{option.name}</Tooltip.Content>
  </Tooltip.Root>
{/snippet}

{#if selection}
  {#if choices.length <= 10}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger {disabled}>
        {#snippet child({ props })}{@render trigger(props)}{/snippet}
      </DropdownMenu.Trigger>
      <DropdownMenu.Content side="top" align="start">
        <DropdownMenu.RadioGroup value={selection.current_value} onValueChange={onSelect}>
          {#each choices as choice (choice.value)}
            <DropdownMenu.RadioItem value={choice.value}>{choice.name}</DropdownMenu.RadioItem>
          {/each}
        </DropdownMenu.RadioGroup>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {:else}
    <Popover.Root bind:open>
      <Popover.Trigger {disabled}>
        {#snippet child({ props })}{@render trigger(props)}{/snippet}
      </Popover.Trigger>
      <Popover.Content side="top" align="start" class="w-80 p-0">
        <Command.Root class="rounded-lg bg-transparent">
          <Command.Input placeholder={`Search ${option.name.toLowerCase()}…`} />
          <Command.List class="max-h-72 overflow-y-auto">
            <Command.Empty>No matches</Command.Empty>
            {#each groups as group (group.heading)}
              <Command.Group heading={group.heading}>
                {#each group.choices as choice (choice.value)}
                  <Command.Item
                    value={choice.value}
                    keywords={[choice.name, choice.group ?? '']}
                    onSelect={() => select(choice.value)}
                  >
                    <div class="flex min-w-0 flex-1 flex-col">
                      <span class="text-sm">{choice.name}</span>
                      {#if choice.description}
                        <span class="truncate text-xs text-subtle">{choice.description}</span>
                      {/if}
                    </div>
                    {#if choice.value === selection.current_value}
                      <Check size={12} class="size-3!" />
                    {/if}
                  </Command.Item>
                {/each}
              </Command.Group>
            {/each}
          </Command.List>
        </Command.Root>
      </Popover.Content>
    </Popover.Root>
  {/if}
{/if}
