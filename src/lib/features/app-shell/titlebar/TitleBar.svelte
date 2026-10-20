<script lang="ts">
  import { platform } from '$lib/platform'
  import { IconButton } from '$lib/components/ui/button'
  import {
    RAIL_WIDTH_CLASS,
    attachSidebarHeaderSlot,
    getShownSidebarFolder,
    getSidebarColumnWidth
  } from '$lib/features/app-shell/sidebar/state.svelte'
  import AppInfo from './AppInfo.svelte'
  import TitleTabStrip from './TitleTabStrip.svelte'
  import WindowControls from './WindowControls.svelte'
  import { isCommandPaletteOpen, setCommandPaletteOpen } from '$lib/features/command-palette/state.svelte'
  import { CommandIcon } from '$lib/icons/lucideExports'
  import { getCommandShortcut } from '$lib/features/command-palette/shortcuts/registry.svelte'
</script>

<header class="flex min-h-9 shrink-0 items-center border-b border-edge bg-surface">
  <!-- Width matches the sidebar rail below so the dividers line up. -->
  <div class="flex {RAIL_WIDTH_CLASS} shrink-0 items-center justify-center self-stretch border-r border-edge">
    <AppInfo />
  </div>

  {#if getShownSidebarFolder()}
    <!-- Spans the sidebar panel plus its 1px resize divider, so this border sits on the divider. -->
    <div
      class="shrink-0 self-stretch border-r border-edge"
      style:width="{getSidebarColumnWidth()}px"
      {@attach attachSidebarHeaderSlot}
    ></div>
  {/if}

  <TitleTabStrip />

  <div class="flex shrink-0 items-center gap-0.5 px-1">
    <IconButton
      size="md"
      tooltip="Commands"
      shortcut={getCommandShortcut('toggle-command-palette')}
      active={isCommandPaletteOpen()}
      onclick={() => setCommandPaletteOpen(true)}
    >
      <CommandIcon size={14} />
    </IconButton>
  </div>

  {#if platform.native}
    <WindowControls />
  {/if}
</header>
