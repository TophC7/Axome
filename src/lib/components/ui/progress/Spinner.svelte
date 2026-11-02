<!--
  @component
  Spinner: the one loading indicator, a spinning refresh glyph. Paints in the
  current text color, so callers tint it with a text class. Pass `label` when
  the spinner is the only loading cue.
-->

<script lang="ts">
  import { RefreshCwIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { cn } from '$lib/utils/cn'

  let { size = 13, label, class: className }: { size?: number; label?: string; class?: string } = $props()
</script>

<!-- The paths spin around the viewBox center (12,12), never the element box:
     at fractional display scales WebKit snaps a spinning box's layer to device
     pixels, so a box at a subpixel layout position swings up to ~1.5px per turn.
     In-svg rotation is rasterized in place and stays centered. -->
<Icon
  icon={RefreshCwIcon}
  data-slot="spinner"
  {size}
  role={label ? 'status' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  class={cn('shrink-0 *:origin-[12px_12px] *:animate-spin', className)}
/>
