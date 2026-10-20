<!--
  @component
  SidebarPanel — sidebar content. Its header (title, actions, extra) renders
  in the title bar's header cell through a portal, so the header snippets
  stay in this component's tree and keep its contexts.

  @param title - panel heading (uppercase label in the title bar)
  @param headerActions - snippet rendered right of the title (e.g. view action buttons)
  @param headerExtra - snippet rendered after headerActions (e.g. info tooltip)
  @param children - main content area
-->

<script lang="ts">
  import PanelHeader from '$lib/components/layout/PanelHeader.svelte'
  import { Portal } from '$lib/components/ui/portal'
  import { getSidebarHeaderSlot } from '$lib/features/app-shell/sidebar/state.svelte'
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

  let slot = $derived(getSidebarHeaderSlot())
</script>

<!-- Portal throws in DEV on a null target; the slot appears once TitleBar attaches its cell. -->
{#if slot}
  <Portal to={slot}>
    <PanelHeader class="h-full min-h-0 border-b-0 bg-transparent pr-1.5">
      {#snippet left()}<span class="truncate text-xs font-semibold tracking-wide text-muted uppercase" {title}
          >{title}</span
        >{/snippet}
      {#snippet right()}
        {#if headerActions}<div class="flex items-center gap-0.5">{@render headerActions()}</div>{/if}
        {@render headerExtra?.()}
      {/snippet}
    </PanelHeader>
  </Portal>
{/if}

<aside class="flex h-full flex-col bg-ground">
  <div class="min-h-0 flex-1 overflow-hidden">
    {@render children?.()}
  </div>
</aside>
