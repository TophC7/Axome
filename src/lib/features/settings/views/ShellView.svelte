<script lang="ts">
  import { Badge } from '$lib/components/ui/badge'
  import { Input } from '$lib/components/ui/input'
  import { notify } from '$lib/features/notifications/state.svelte'
  import { getSettings, getSettingsHost, saveShellConfig } from '$lib/features/settings/state/settings.svelte'
  import type { ShellConfig } from '$lib/types/backend'
  import { getErrorMessage } from '$lib/utils/client-error'
  import { onDestroy } from 'svelte'
  import { createAutoSaver } from './autoSaver'

  let { onSaving, onSaved }: { onSaving: () => void; onSaved: () => void } = $props()

  let settings = $derived(getSettings())
  const host = getSettingsHost()
  type Draft = { binaryPath: string; extraArgs: string }
  // Keep edits local so a backend refresh cannot replace pending input.
  let draft = $state<Draft | null>(null)
  let values = $derived(
    draft ?? {
      binaryPath: settings?.shell.binary_path_override ?? '',
      extraArgs: settings?.shell.extra_args.join(' ') ?? ''
    }
  )

  const saver = createAutoSaver({
    onBusyChange: (busy) => (busy ? onSaving() : onSaved())
  })
  onDestroy(() => saver.dispose())

  function update(key: keyof Draft, value: string) {
    draft = { ...values, [key]: value }
    const next: ShellConfig = {
      binary_path_override: draft.binaryPath.trim() || null,
      extra_args: draft.extraArgs.split(/\s+/).filter(Boolean)
    }
    saver.schedule('shell', async () => {
      try {
        await saveShellConfig(next, host)
      } catch (error) {
        notify.error('Save shell failed', getErrorMessage(error))
      }
    })
  }
</script>

<section class="border-b border-edge px-5 py-4">
  <div class="flex flex-col gap-3">
    <h4 class="text-sm font-semibold text-bright">Shell</h4>
    <label class="flex items-center">
      <span class="w-36 shrink-0 text-sm text-muted">Shell path</span>
      <Input
        class="flex-1 py-2"
        value={values.binaryPath}
        oninput={(e: Event) => update('binaryPath', (e.currentTarget as HTMLInputElement).value)}
        placeholder="Detected login shell"
        disabled={!settings}
      />
    </label>
    <label class="flex items-center">
      <span class="w-36 shrink-0 text-sm text-muted">Shell args</span>
      <Input
        class="flex-1 py-2"
        value={values.extraArgs}
        oninput={(e: Event) => update('extraArgs', (e.currentTarget as HTMLInputElement).value)}
        placeholder="--flag value"
        disabled={!settings}
      />
    </label>
  </div>
</section>

<section class="border-b border-edge px-5 py-4">
  <div class="flex flex-col gap-3">
    <h4 class="text-sm font-semibold text-bright">Presets</h4>
    {#each Object.entries(settings?.shell.presets ?? {}) as [name, preset] (name)}
      <div class="flex flex-col gap-1 border-b border-edge pb-3">
        <div class="flex items-center gap-2">
          <span class="font-mono text-sm text-fg">{name}</span>
          {#if name === settings?.shell.default_preset}
            <Badge>Default</Badge>
          {/if}
        </div>
        <div class="font-mono text-xs break-all text-muted">
          {preset.command ?? 'default shell'}{preset.args.length ? ` ${preset.args.join(' ')}` : ''}
        </div>
        {#if preset.startup}
          <div class="font-mono text-xs break-all whitespace-pre-wrap text-muted">{preset.startup}</div>
        {/if}
      </div>
    {/each}
    <p class="text-xs text-subtle">
      Add presets under <code>shell.presets</code> and pick one with
      <code>shell.default_preset</code> in settings.jsonc.
    </p>
  </div>
</section>
