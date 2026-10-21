<script lang="ts">
  import { Collapsible as CollapsiblePrimitive } from 'bits-ui'
  import { cn } from '$lib/utils/cn'

  let { ref = $bindable(null), class: className, ...rest }: CollapsiblePrimitive.ContentProps = $props()
</script>

<CollapsiblePrimitive.Content bind:ref data-slot="collapsible-content" class={cn(className)} {...rest} />
