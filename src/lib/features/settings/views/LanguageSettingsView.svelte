<script lang="ts">
  import { Badge } from '$lib/components/ui/badge'
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import { Switch } from '$lib/components/ui/switch'
  import * as Tabs from '$lib/components/ui/tabs'
  import { AlertCircleIcon, ChevronDownIcon, RefreshCwIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { getLspServers, getLspServersLoading, loadLspServers } from '$lib/features/settings/state/lspSettings.svelte'
  import { getFormatterStatuses, loadFormatterStatuses } from '$lib/features/settings/state/formatterSettings.svelte'
  import { getFormatterPolicy } from '$lib/features/extensions/catalog.svelte'
  import { notify } from '$lib/features/notifications/state.svelte'
  import {
    getSettings,
    getSettingsHost,
    saveFormattingSettings,
    saveLspServerConfig
  } from '$lib/features/settings/state/settings.svelte'
  import { getActiveFolderPath } from '$lib/features/workbench/state.svelte'
  import type {
    SettingsPage,
    LanguageFormatterPolicy,
    LspServerConfig,
    LspServerSettingsEntry
  } from '$lib/types/backend'
  import { getErrorMessage } from '$lib/utils/client-error'
  import { onDestroy, untrack } from 'svelte'
  import { createTrackedAsyncLoad } from '$lib/utils/trackedAsyncLoad.svelte'
  import { createAutoSaver } from './autoSaver'

  type StatusHook = () => void

  let {
    definition,
    onSaving,
    onSaved
  }: {
    definition: SettingsPage
    onSaving: StatusHook
    onSaved: StatusHook
  } = $props()

  let settings = $derived(getSettings())
  const host = getSettingsHost()
  let activeFolderPath = $derived(getActiveFolderPath() ?? undefined)
  let lspLoading = $derived(getLspServersLoading())
  let allServers = $derived(getLspServers())
  let servers = $derived(
    allServers.filter((entry) => definition.server_definition_ids.includes(entry.server.server_definition_id))
  )
  let policies = $derived(definition.language_ids.flatMap((id) => getFormatterPolicy(host, id) ?? []))
  let formatterStatuses = $derived(getFormatterStatuses())

  type Draft = {
    enabled: boolean
    binaryPath: string
    extraArgs: string
    trace: 'off' | 'messages' | 'verbose'
    settings: unknown | null
  }

  let drafts = $state<Record<string, Draft>>({})
  let expanded = $state<Record<string, boolean>>({})

  // Editable drafts belong to the loaded host's global settings, never the
  // active folder's effective runtime configuration.
  $effect(() => {
    const globalSettings = settings
    const ids = definition.server_definition_ids
    if (!globalSettings) return
    untrack(() => {
      for (const id of ids) {
        if (id in drafts) continue
        const config = globalSettings.lsp.servers[id]
        if (!config) throw new Error(`Missing global settings for language server: ${id}`)
        drafts[id] = {
          enabled: config.enabled,
          binaryPath: config.binary_path_override ?? '',
          extraArgs: config.extra_args.join(' '),
          trace: config.trace,
          settings: config.settings ?? null
        }
      }
    })
  })

  const saver = createAutoSaver({
    onBusyChange: (busy) => (busy ? onSaving() : onSaved())
  })

  // Folder switches refresh runtime status; host-keyed remounts own drafts.
  const lspLoader = createTrackedAsyncLoad<string | undefined>()
  $effect(() => {
    const folderPath = activeFolderPath
    lspLoader.run(folderPath, () => loadLspServers(folderPath))
  })

  const formatterLoader = createTrackedAsyncLoad<string | undefined>()
  $effect(() => {
    const folderPath = activeFolderPath
    formatterLoader.run(folderPath, () => loadFormatterStatuses(folderPath))
  })

  onDestroy(() => saver.dispose())

  function selectedFormatter(policy: LanguageFormatterPolicy): string {
    const stored = settings?.formatting[policy.language_id]?.formatter
    return stored && policy.options.some((option) => option.id === stored) ? stored : policy.default
  }

  async function saveFormatter(policy: LanguageFormatterPolicy, formatter: string) {
    if (!settings || selectedFormatter(policy) === formatter) return
    onSaving()
    try {
      await saveFormattingSettings(
        {
          ...settings.formatting,
          [policy.language_id]: { formatter }
        },
        host
      )
    } catch (error) {
      notify.error('Save formatting settings failed', getErrorMessage(error))
    } finally {
      onSaved()
    }
  }

  async function saveServer(snapshot: LspServerConfig, host: string | null) {
    try {
      await saveLspServerConfig(snapshot, host)
    } catch (error) {
      notify.error('Save language server failed', getErrorMessage(error))
    }
  }

  function updateServer(id: string, key: keyof Draft, value: boolean | string) {
    const draft = drafts[id]
    if (!draft) return
    const next = { ...draft, [key]: value }
    drafts = { ...drafts, [id]: next }
    const snapshot: LspServerConfig = {
      server_definition_id: id,
      enabled: next.enabled,
      binary_path_override: next.binaryPath.trim() || null,
      extra_args: splitArgs(next.extraArgs),
      trace: next.trace,
      settings: next.settings
    }
    saver.schedule(id, () => saveServer(snapshot, host))
  }

  function toggleExpanded(id: string) {
    expanded = { ...expanded, [id]: !(expanded[id] ?? false) }
  }

  async function refresh() {
    try {
      await Promise.all([loadLspServers(activeFolderPath), loadFormatterStatuses(activeFolderPath)])
    } catch (error) {
      notify.error('Refresh language servers failed', getErrorMessage(error))
    }
  }

  function statusVariant(status: string): 'success' | 'warning' | 'danger' | 'default' {
    if (status === 'connected') return 'success'
    if (status === 'missing') return 'warning'
    if (status === 'error') return 'danger'
    return 'default'
  }

  function splitArgs(value: string): string[] {
    return value
      .split(/\s+/)
      .map((segment) => segment.trim())
      .filter(Boolean)
  }

  function relatedLanguages(entry: LspServerSettingsEntry): string[] {
    const pageLanguages = new Set(definition.language_ids)
    const languages = new Set<string>()
    for (const selector of entry.server.document_selectors) {
      if (selector.language && !pageLanguages.has(selector.language)) {
        languages.add(selector.language)
      }
    }
    return [...languages].sort()
  }
</script>

<section class="border-b border-edge px-5 py-4">
  <div class="flex flex-col gap-2">
    <h4 class="text-sm font-semibold text-bright">Formatting</h4>

    {#if policies.length > 0}
      {#if settings}
        {#each policies as policy (policy.language_id)}
          {@const selected = selectedFormatter(policy)}
          {@const status = formatterStatuses.find((entry) => entry.formatter_id === selected)}
          <div class="flex min-w-0 flex-col gap-2">
            {#if policies.length > 1}
              <p class="text-xs text-muted">{policy.language_label}</p>
            {/if}
            <Tabs.Root value={selected} onValueChange={(value) => saveFormatter(policy, value)}>
              <Tabs.List>
                {#each policy.options as option (option.id)}
                  <Tabs.Trigger value={option.id}>{option.label}</Tabs.Trigger>
                {/each}
              </Tabs.List>
            </Tabs.Root>
            {#if status}
              <div class="flex min-w-0 flex-col gap-0.5">
                <div class="flex items-center gap-2">
                  <Badge variant={status.status === 'available' ? 'success' : 'warning'}>{status.status}</Badge>
                </div>
                <span class="truncate font-mono text-xs text-muted">
                  {status.resolved_path ?? status.install_hint}
                </span>
              </div>
              {#if status.status === 'missing' && status.message}
                <div class="flex items-center gap-2 bg-danger-bg/40 px-5 py-2 text-sm text-danger-bright">
                  <Icon icon={AlertCircleIcon} size={14} />
                  <span>{status.message}</span>
                </div>
              {/if}
            {/if}
          </div>
        {/each}
      {:else}
        <p class="text-xs text-subtle">Loading formatting settings&hellip;</p>
      {/if}
    {:else}
      <p class="text-xs text-subtle">Formatting currently follows the language server path.</p>
      <div>
        <Badge>LSP</Badge>
      </div>
    {/if}
  </div>
</section>

<section class="flex items-start justify-between gap-4 px-5 py-4">
  <h4 class="text-sm font-semibold text-bright">Language servers</h4>

  <Button variant="outline" size="xs" onclick={refresh}>
    <Icon icon={RefreshCwIcon} size={12} />
    Refresh
  </Button>
</section>

{#if lspLoading && servers.length === 0}
  <div class="border-t border-edge px-5 py-8 text-sm text-muted">Loading language servers&hellip;</div>
{:else if servers.length === 0}
  <div class="border-t border-edge px-5 py-8 text-sm text-muted">
    No language servers are attached to this language yet.
  </div>
{:else}
  <div class="flex flex-col border-t border-edge">
    {#each servers as entry (entry.server.server_definition_id)}
      {@const id = entry.server.server_definition_id}
      {@const draft = drafts[id]}
      {@const open = expanded[id] ?? false}
      {@const extraLanguages = relatedLanguages(entry)}

      <div class="border-b border-edge/70">
        <div
          role="button"
          tabindex="0"
          onclick={() => toggleExpanded(id)}
          onkeydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') {
              event.preventDefault()
              toggleExpanded(id)
            }
          }}
          class="group flex cursor-pointer items-center gap-3 px-5 py-3 hover:bg-surface/40"
        >
          <div
            role="presentation"
            onclick={(event) => event.stopPropagation()}
            onkeydown={(event) => event.stopPropagation()}
          >
            <Switch
              checked={draft?.enabled ?? true}
              disabled={!settings || !draft}
              onCheckedChange={(value) => updateServer(id, 'enabled', value)}
            />
          </div>

          <div class="flex min-w-0 flex-1 flex-col gap-0.5">
            <div class="flex items-center gap-2">
              <span class="text-sm font-semibold text-bright">{entry.server.label}</span>
              <Badge variant={statusVariant(entry.server.status)}>{entry.server.status}</Badge>
            </div>
            <span class="truncate font-mono text-xs text-muted">
              {entry.server.resolved_path ?? entry.server.install_hint}
            </span>
            {#if extraLanguages.length > 0}
              <span class="text-xs text-subtle">Shared config</span>
            {/if}
          </div>

          <Icon
            icon={ChevronDownIcon}
            size={14}
            class="shrink-0 text-subtle transition-transform group-hover:text-fg {open ? 'rotate-180' : ''}"
          />
        </div>

        {#if entry.server.message}
          <div class="flex items-center gap-2 bg-danger-bg/40 px-5 py-2 text-sm text-danger-bright">
            <Icon icon={AlertCircleIcon} size={14} />
            <span>{entry.server.message}</span>
          </div>
        {/if}

        {#if open && draft}
          <div class="flex flex-col gap-3 bg-surface/40 px-5 py-4">
            <div class="flex items-center gap-3">
              <span class="w-32 shrink-0 text-sm text-muted">Binary path</span>
              <Input
                class="flex-1 py-2"
                value={draft.binaryPath}
                oninput={(event: Event) =>
                  updateServer(id, 'binaryPath', (event.currentTarget as HTMLInputElement).value)}
                placeholder="Use detected binary"
              />
            </div>

            <div class="flex items-center gap-3">
              <span class="w-32 shrink-0 text-sm text-muted">Extra args</span>
              <Input
                class="flex-1 py-2"
                value={draft.extraArgs}
                oninput={(event: Event) =>
                  updateServer(id, 'extraArgs', (event.currentTarget as HTMLInputElement).value)}
                placeholder="--stdio already comes from the manifest"
              />
            </div>

            <div class="flex items-start gap-3">
              <span class="w-32 shrink-0 pt-1 text-sm text-muted">Trace</span>
              <div class="flex flex-wrap gap-2">
                {#each ['off', 'messages', 'verbose'] as trace}
                  <Button
                    variant={draft.trace === trace ? 'outline' : 'ghost'}
                    size="xs"
                    onclick={() => updateServer(id, 'trace', trace)}
                  >
                    {trace}
                  </Button>
                {/each}
              </div>
            </div>

            <div class="flex items-start gap-3">
              <span class="w-32 shrink-0 pt-1.5 text-sm text-muted">Preferences</span>
              <p class="text-xs text-subtle">
                Edit nested LSP preferences in <span class="font-mono">settings.jsonc</span> under
                <span class="font-mono">lsp.servers.{id}.settings</span>.
              </p>
            </div>
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
