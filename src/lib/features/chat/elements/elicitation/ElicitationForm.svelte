<script lang="ts">
  import type { HTMLAttributes } from 'svelte/elements'
  import { Button } from '$lib/components/ui/button'
  import { Input, Select } from '$lib/components/ui/input'
  import { Checkbox } from '$lib/components/ui/checkbox'
  import { ToggleChip } from '$lib/components/ui/toggle-chip'
  import { cn } from '$lib/utils/cn'

  type Value = string | number | boolean | (string | number | boolean)[]
  type Choice = { value: string | number | boolean; label: string }
  let {
    message,
    schema,
    onRespond,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    message: string
    schema: unknown
    onRespond: (response: { action: 'accept' | 'decline' | 'cancel'; content?: Record<string, Value> }) => void
  } = $props()
  const id = $props.id()
  let values = $state<Record<string, Value>>({})
  function object(value: unknown): Record<string, unknown> {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {}
  }
  function choices(field: Record<string, unknown>): Choice[] | null {
    const variants = field.oneOf ?? field.anyOf
    if (Array.isArray(variants)) {
      const result: Choice[] = []
      for (const variant of variants) {
        const item = object(variant)
        if (typeof item.const !== 'string' && typeof item.const !== 'number' && typeof item.const !== 'boolean')
          return null
        result.push({ value: item.const, label: typeof item.title === 'string' ? item.title : String(item.const) })
      }
      return result.length ? result : null
    }
    if (!Array.isArray(field.enum) || !field.enum.length) return null
    const result: Choice[] = []
    for (const [index, value] of field.enum.entries()) {
      if (typeof value !== 'string' && typeof value !== 'number' && typeof value !== 'boolean') return null
      const label = Array.isArray(field.enumNames) ? field.enumNames[index] : undefined
      result.push({ value, label: typeof label === 'string' ? label : String(value) })
    }
    return result
  }
  const fields = $derived(
    Object.entries(object(object(schema).properties)).map(([key, raw]) => {
      const field = object(raw)
      const options = choices(field.type === 'array' ? object(field.items) : field)
      const supported =
        field.type === 'string'
          ? !(field.oneOf || field.enum) ||
            (options !== null && options.every((option) => typeof option.value === 'string'))
          : field.type === 'number' ||
            field.type === 'integer' ||
            field.type === 'boolean' ||
            (field.type === 'array' && options !== null)
      return { key, field, options, supported, title: typeof field.title === 'string' ? field.title : key }
    })
  )
  const required = $derived(Array.isArray(object(schema).required) ? (object(schema).required as unknown[]) : [])
  function filled(value: Value | undefined): boolean {
    return (
      value !== undefined &&
      (typeof value === 'string' ? value.trim() !== '' : Array.isArray(value) ? value.length > 0 : true)
    )
  }
  const invalid = $derived(
    fields.some(({ key, field, supported }) => {
      if (!supported) return true
      const value = values[key]
      return (
        typeof value === 'number' &&
        (!Number.isFinite(value) ||
          (field.type === 'integer' && !Number.isInteger(value)) ||
          (typeof field.minimum === 'number' && value < field.minimum) ||
          (typeof field.maximum === 'number' && value > field.maximum))
      )
    }) || required.some((key) => typeof key !== 'string' || !filled(values[key]))
  )
  function setNumber(key: string, input: HTMLInputElement) {
    if (input.value === '') delete values[key]
    else values[key] = input.valueAsNumber
  }
  function toggle(key: string, value: Choice['value'], checked: boolean) {
    const selected = values[key]
    const current = Array.isArray(selected) ? selected : []
    values[key] = checked ? [...current, value] : current.filter((item) => item !== value)
  }
  function accept() {
    if (invalid) return
    const content = Object.fromEntries(Object.entries(values).filter(([, value]) => filled(value)))
    onRespond({ action: 'accept', content })
  }
</script>

<div
  data-slot="elicitation-form"
  class={cn('flex flex-col gap-3 rounded-lg border border-accent/40 bg-surface p-3', className)}
  {...rest}
>
  <p class="text-sm text-fg">{message}</p>
  {#each fields as { key, field, options, supported, title } (key)}
    <div class="flex flex-col gap-1.5">
      <label id={`${id}-${key}-label`} for={`${id}-${key}`} class="text-sm text-fg">{title}</label>
      {#if !supported}
        <p class="text-xs text-subtle">Unsupported field</p>
      {:else if field.type === 'string' && options}
        {#if options.length <= 6}
          <div role="radiogroup" aria-labelledby={`${id}-${key}-label`} class="flex flex-wrap gap-1.5">
            {#each options as option}
              <ToggleChip
                role="radio"
                aria-checked={values[key] === option.value}
                pressed={values[key] === option.value}
                class="font-sans text-xs"
                onclick={() => (values[key] = option.value)}>{option.label}</ToggleChip
              >
            {/each}
          </div>
        {:else}
          <Select
            id={`${id}-${key}`}
            value={typeof values[key] === 'string' ? String(values[key]) : ''}
            onchange={(event) => (values[key] = event.currentTarget.value)}
          >
            <option value="" disabled>Select an option</option>
            {#each options as option}<option value={String(option.value)}>{option.label}</option>{/each}
          </Select>
        {/if}
      {:else if field.type === 'string'}
        <Input
          id={`${id}-${key}`}
          value={typeof values[key] === 'string' ? String(values[key]) : ''}
          oninput={(event) => (values[key] = event.currentTarget.value)}
        />
      {:else if field.type === 'number' || field.type === 'integer'}
        <Input
          id={`${id}-${key}`}
          type="number"
          step={field.type === 'integer' ? 1 : 'any'}
          min={typeof field.minimum === 'number' ? field.minimum : undefined}
          max={typeof field.maximum === 'number' ? field.maximum : undefined}
          value={values[key] as number | undefined}
          oninput={(event) => setNumber(key, event.currentTarget)}
        />
      {:else if field.type === 'boolean'}
        <Checkbox
          id={`${id}-${key}`}
          checked={values[key] === true}
          onCheckedChange={(checked) => (values[key] = checked)}
        />
      {:else if field.type === 'array' && options}
        <div role="group" aria-labelledby={`${id}-${key}-label`} class="flex flex-col gap-1.5">
          {#each options as option, index}
            <label class="flex items-center gap-2 text-sm text-fg" for={`${id}-${key}-${index}`}>
              <Checkbox
                id={`${id}-${key}-${index}`}
                checked={Array.isArray(values[key]) && (values[key] as Choice['value'][]).includes(option.value)}
                onCheckedChange={(checked) => toggle(key, option.value, checked)}
              />
              {option.label}
            </label>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
  <div class="flex gap-1.5">
    <Button size="xs" variant="accent" disabled={invalid} onclick={accept}>Accept</Button>
    <Button size="xs" variant="outline" onclick={() => onRespond({ action: 'decline' })}>Decline</Button>
    <Button size="xs" variant="ghost" onclick={() => onRespond({ action: 'cancel' })}>Cancel</Button>
  </div>
</div>
