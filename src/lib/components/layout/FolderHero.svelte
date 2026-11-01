<script lang="ts">
  import type { Snippet } from 'svelte'
  import { BlurFade } from '$lib/components/ui/blur-fade'
  import { ChevronRight } from '$lib/icons/lucideExports'
  import { cn } from '$lib/utils/cn'
  import { getPathColor } from '$lib/utils/pathColor'
  import { basename, splitRemotePath } from '$lib/utils/paths'

  let { folderPath, class: className, children }: { folderPath: string; class?: string; children?: Snippet } = $props()

  const path = $derived(splitRemotePath(folderPath)?.path ?? folderPath)
</script>

<BlurFade delay={0.05} duration={0.5} offset={10}>
  <header class={cn('flex max-w-full flex-col items-center gap-1 text-center', className)}>
    <!-- The chevron hangs left of the name so the name alone is centered. -->
    <h2 class="relative max-w-full text-3xl text-bright">
      <ChevronRight
        size={28}
        class="absolute top-1/2 right-full -translate-y-1/2"
        style="color: {getPathColor(folderPath)}"
        aria-hidden="true"
      />
      <span class="block truncate">{basename(path) || '/'}</span>
    </h2>
    <p class="max-w-full truncate font-mono text-sm text-subtle" title={folderPath}>{path}</p>
    {@render children?.()}
  </header>
</BlurFade>
