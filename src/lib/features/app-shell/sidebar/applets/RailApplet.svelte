<!--
  @component
  RailApplet — a panel applet docked at the foot of the sidebar rail: a
  RailButton that opens a small panel to the right with a header (icon tile,
  label, status line), a body and an action row.

  Every rail-foot applet (panel or not) follows one contract:
  - It renders nothing when it has nothing to offer.
  - It exposes its state as one `ToneStatus`. The corner dot (unless idle),
    the tooltip "<label> · <text>", the aria name and the panel's status line
    all derive from it, so state is never shown by colour alone.
  - It stays cheap while closed: work it owns runs only while `open`.
    Per-folder activation belongs to WorkbenchView, not the applet.
  - Panel rows use `appletRowClass`; section labels use `appletHeadingClass`.

  @param label - applet name: rail tooltip, aria name, panel title
  @param status - the applet's current state
  @param icon - 16px icon snippet; rendered in the rail button and again in the panel header tile
  @param children - panel body
  @param actions - footer buttons, right-aligned
  @param open - bindable; applets use it to do work only while shown
-->

<script lang="ts" module>
  import { cn } from '$lib/utils/cn'
  import { menuGroupHeadingClass } from '$lib/components/ui/menu-recipe'

  // A row highlights like a menu item (raised to surface); no hover transition, it is a swept list.
  export const appletRowClass =
    'flex h-7 w-full cursor-pointer items-center gap-2 rounded-sm px-2 text-left text-sm text-fg outline-none hover:bg-surface focus-visible:shadow-focus-ring disabled:cursor-default disabled:opacity-50'

  export const appletHeadingClass = cn(menuGroupHeadingClass, 'px-2')
</script>

<script lang="ts">
  import * as Popover from '$lib/components/ui/popover'
  import { statusToneText, type ToneStatus } from '$lib/components/ui/status-dot'
  import RailButton from '../RailButton.svelte'
  import type { Snippet } from 'svelte'

  let {
    label,
    status,
    icon,
    children,
    actions,
    open = $bindable(false)
  }: {
    label: string
    status: ToneStatus
    icon: Snippet
    children?: Snippet
    actions?: Snippet
    open?: boolean
  } = $props()
</script>

<Popover.Root bind:open>
  <Popover.Trigger>
    {#snippet child({ props })}
      <RailButton {...props} {label} {status} active={open}>{@render icon()}</RailButton>
    {/snippet}
  </Popover.Trigger>

  <Popover.Content side="right" align="end" sideOffset={10} class="w-72 overflow-hidden p-0">
    <div class="flex items-center gap-2.5 border-b border-edge bg-surface/60 px-3 py-2.5">
      <div class="flex size-8 shrink-0 items-center justify-center rounded-lg border border-edge bg-ground text-fg">
        {@render icon()}
      </div>
      <div class="min-w-0 flex-1">
        <div class="truncate text-md text-bright">{label}</div>
        <div class="truncate text-xs {statusToneText[status.tone]}">{status.text}</div>
      </div>
    </div>

    {#if children}
      <div class="p-2">{@render children()}</div>
    {/if}

    {#if actions}
      <div class="flex items-center justify-end gap-1.5 border-t border-edge px-3 py-2">
        {@render actions()}
      </div>
    {/if}
  </Popover.Content>
</Popover.Root>
