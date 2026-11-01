<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import * as Popover from '$lib/components/ui/popover'
  import * as Command from '$lib/components/ui/command'
  import { Check, ChevronDown } from '$lib/icons/lucideExports'

  let {
    label,
    value,
    choices,
    disabled = false,
    open = $bindable(false),
    onSelect
  }: {
    label: string
    value: string
    choices: { value: string; name: string; group?: string }[]
    disabled?: boolean
    open?: boolean
    onSelect: (value: string) => void
  } = $props()

  const currentName = $derived(choices.find((choice) => choice.value === value)?.name ?? value)
  const groups = $derived.by(() => {
    const result = new Map<string | undefined, typeof choices>()
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
          aria-label={label}
          {disabled}
          class="max-w-48 hover:bg-raised"
        >
          <span class="truncate">{currentName}</span>
          <ChevronDown size={12} />
        </Button>
      {/snippet}
    </Tooltip.Trigger>
    <Tooltip.Content>{label}</Tooltip.Content>
  </Tooltip.Root>
{/snippet}

{#if choices.length <= 10}
  <DropdownMenu.Root bind:open>
    <DropdownMenu.Trigger {disabled}>
      {#snippet child({ props })}{@render trigger(props)}{/snippet}
    </DropdownMenu.Trigger>
    <DropdownMenu.Content side="top" align="end">
      <DropdownMenu.RadioGroup {value} onValueChange={select}>
        {#each groups as group (group.heading)}
          <DropdownMenu.Group>
            {#if group.heading}<DropdownMenu.GroupHeading>{group.heading}</DropdownMenu.GroupHeading>{/if}
            {#each group.choices as choice (choice.value)}
              <DropdownMenu.RadioItem value={choice.value}>{choice.name}</DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.Group>
        {/each}
      </DropdownMenu.RadioGroup>
    </DropdownMenu.Content>
  </DropdownMenu.Root>
{:else}
  <Popover.Root bind:open>
    <Popover.Trigger {disabled}>
      {#snippet child({ props })}{@render trigger(props)}{/snippet}
    </Popover.Trigger>
    <Popover.Content side="top" align="end" class="w-80 p-0">
      <Command.Root class="rounded-lg bg-transparent">
        <Command.Input placeholder={`Search ${label.toLowerCase()}…`} />
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
                  </div>
                  {#if choice.value === value}
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
