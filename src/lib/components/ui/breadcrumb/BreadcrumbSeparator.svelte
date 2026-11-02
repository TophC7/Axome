<script lang="ts">
  import { ChevronRightIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { cn } from '$lib/utils/cn'
  import type { HTMLLiAttributes } from 'svelte/elements'

  let {
    class: className,
    ...rest
  }: HTMLLiAttributes & {
    class?: string
  } = $props()
</script>

<li role="presentation" aria-hidden="true" class={cn('inline-flex items-center text-subtle', className)} {...rest}>
  <Icon icon={ChevronRightIcon} size={12} />
</li>
