<script lang="ts">
  import type { Snippet } from 'svelte'
  import type { HTMLAttributes } from 'svelte/elements'
  import { BlurFade } from '$lib/components/ui/blur-fade'
  import { cn } from '$lib/utils/cn'

  let {
    title,
    description,
    icon,
    class: className,
    ...rest
  }: HTMLAttributes<HTMLDivElement> & {
    title: string
    description: string
    icon?: Snippet
  } = $props()
</script>

<div data-slot="conversation-empty-state" class={cn('my-auto w-full', className)} {...rest}>
  <BlurFade>
    <div class="flex flex-col items-center gap-2 py-8 text-center">
      {#if icon}
        <div class="mb-1 text-subtle [&_svg]:size-7">{@render icon()}</div>
      {/if}
      <h2 class="text-lg text-bright">{title}</h2>
      <p class="text-sm text-muted">{description}</p>
    </div>
  </BlurFade>
</div>
