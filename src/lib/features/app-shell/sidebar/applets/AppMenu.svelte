<!--
  @component
  AppMenu — the app menu behind the Settings icon at the foot of the sidebar
  rail, rendered from buildAppMenu. Settings files that fail to parse or
  validate mark the icon and list at the top of the menu.
-->

<script lang="ts">
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import type { ToneStatus } from '$lib/components/ui/status-dot'
  import { TriangleAlertIcon, Settings01Icon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { getSettingsDiagnostics } from '$lib/features/settings/state/diagnostics.svelte'
  import RailButton from '../RailButton.svelte'
  import { buildAppMenu } from './appMenu'

  const SHOWN_DIAGNOSTICS = 5

  let open = $state(false)
  let entries = $derived(buildAppMenu())
  let diagnostics = $derived(getSettingsDiagnostics())
  let status = $derived<ToneStatus | null>(
    diagnostics.length
      ? {
          tone: 'waiting',
          text: `${diagnostics.length} settings issue${diagnostics.length === 1 ? '' : 's'}`
        }
      : null
  )
</script>

<DropdownMenu.Root bind:open>
  <DropdownMenu.Trigger>
    {#snippet child({ props })}
      <RailButton {...props} label="Settings" {status} active={open}
        ><Icon icon={Settings01Icon} size={16} /></RailButton
      >
    {/snippet}
  </DropdownMenu.Trigger>

  <DropdownMenu.Content side="right" align="end" sideOffset={6}>
    {#if status}
      <div class="max-w-80 px-2.5 py-1.5">
        <div class="flex items-center gap-1.5 text-xs font-medium text-warning-bright">
          <Icon icon={TriangleAlertIcon} size={11} />
          {status.text}
        </div>
        {#each diagnostics.slice(0, SHOWN_DIAGNOSTICS) as diagnostic}
          <div class="mt-1 font-mono text-2xs break-all text-muted">
            {diagnostic.origin === 'host' ? 'host ' : ''}{diagnostic.layer}: {diagnostic.path}{diagnostic.pointer
              ? ` ${diagnostic.pointer}`
              : ''}
            <span class="font-sans text-warning-bright">{diagnostic.message}</span>
          </div>
        {/each}
        {#if diagnostics.length > SHOWN_DIAGNOSTICS}
          <div class="mt-1 text-2xs text-subtle">+{diagnostics.length - SHOWN_DIAGNOSTICS} more</div>
        {/if}
      </div>
      <DropdownMenu.Separator />
    {/if}
    {#each entries as entry, i (i)}
      {#if entry.kind === 'separator'}
        <DropdownMenu.Separator />
      {:else}
        <DropdownMenu.Item onSelect={entry.onSelect} disabled={entry.disabled}>
          <span>{entry.label}</span>
          {#if entry.shortcut}
            <DropdownMenu.Shortcut>{entry.shortcut}</DropdownMenu.Shortcut>
          {/if}
        </DropdownMenu.Item>
      {/if}
    {/each}
  </DropdownMenu.Content>
</DropdownMenu.Root>
