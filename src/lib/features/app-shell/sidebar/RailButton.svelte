<!--
  @component
  RailButton — an icon on the sidebar rail: right-side tooltip carrying the
  label and the item's current state, a bottom-right status dot (none when
  idle), `active` pins the hover look. Bits triggers wrap it through their
  `child` snippet.
-->

<script lang="ts">
  import { IconButton } from '$lib/components/ui/button'
  import { StatusDot, type ToneStatus } from '$lib/components/ui/status-dot'
  import type { Snippet } from 'svelte'

  let {
    label,
    status = null,
    active = false,
    children,
    ...rest
  }: {
    label: string
    status?: ToneStatus | null
    active?: boolean
    children: Snippet
    [key: string]: unknown
  } = $props()
</script>

<IconButton
  size="md"
  tooltip={status ? `${label} · ${status.text}` : label}
  tooltipSide="right"
  {active}
  class="relative"
  {...rest}
>
  {@render children()}
  {#if status && status.tone !== 'idle'}<StatusDot tone={status.tone} size="sm" corner />{/if}
</IconButton>
