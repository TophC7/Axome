<!--
  @component
  SidebarPanel — sidebar content under its own header (title, actions, extra).

  @param title - panel heading (uppercase label)
  @param headerActions - snippet rendered right of the title (e.g. view action buttons)
  @param headerExtra - snippet rendered after headerActions (e.g. info tooltip)
  @param children - main content area
-->

<script lang="ts">
  import PanelHeader from '$lib/components/layout/PanelHeader.svelte'
  import type { Snippet } from 'svelte'

  let {
    title,
    headerActions,
    headerExtra,
    children
  }: {
    title: string
    headerActions?: Snippet
    headerExtra?: Snippet
    children?: Snippet
  } = $props()
</script>

<aside class="flex h-full flex-col bg-ground">
  <PanelHeader class="bg-ground pr-1.5">
    {#snippet left()}<span class="truncate text-xs font-semibold tracking-wide text-muted uppercase" {title}
        >{title}</span
      >{/snippet}
    {#snippet right()}
      {#if headerActions}<div class="flex items-center gap-0.5">{@render headerActions()}</div>{/if}
      {@render headerExtra?.()}
    {/snippet}
  </PanelHeader>
  <div class="min-h-0 flex-1 overflow-hidden">
    {@render children?.()}
  </div>
</aside>
