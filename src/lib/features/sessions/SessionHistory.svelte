<!--
  @component
  SessionHistory — the folder's searchable chat and harness session history,
  with per-source status and explicit harness resume presets in Open With.
-->

<script lang="ts">
  import type { Snippet } from 'svelte'
  import * as ContextMenu from '$lib/components/ui/context-menu'
  import { SearchInput } from '$lib/components/ui/input'
  import { SidebarRow } from '$lib/components/ui/sidebar-row'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { HistoryIcon, MessagesSquareIcon } from '$lib/icons/lucideExports'
  import type { IndexedSession } from '$lib/types/backend'
  import { formatFullDate, timeAgo } from '$lib/utils/date'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import { launchLabel, sessionBrand } from '$lib/features/sessions/harnesses/catalog'
  import { openIndexedSession, sessionLaunchPreset } from './service.svelte'
  import { getSessionIndex } from './state.svelte'

  let {
    folderPath,
    rows,
    section
  }: {
    folderPath: string
    rows: IndexedSession[]
    section: Snippet<[string, number, boolean, () => void]>
  } = $props()

  const PAGE = 100
  let query = $state('')
  let limit = $state(PAGE)
  let collapsed = $state(false)

  let index = $derived(getSessionIndex(folderPath))
  let sources = $derived(index.snapshot?.sources ?? [])
  let issues = $derived(sources.filter((source) => source.status === 'error' || source.status === 'missing'))
  let loadingHistory = $derived(
    index.snapshot === null
      ? index.loading
      : rows.length === 0 && sources.some((source) => source.refreshing || source.status === 'pending')
  )
  let cached = $derived(
    rows.length > 0 && (index.loading || sources.some((source) => source.refreshing || source.status === 'error'))
  )
  let needle = $derived(query.trim().toLowerCase())
  let matches = $derived(needle ? rows.filter((session) => searchText(session).includes(needle)) : rows)
  let visible = $derived(matches.slice(0, limit))

  function searchText(session: IndexedSession): string {
    return [
      session.title ?? '',
      session.sessionId,
      ...(session.source.kind === 'harness'
        ? [
            sessionBrand(folderPath, session.source.harnessId).label,
            ...session.source.presets.map((preset) => preset ?? '')
          ]
        : ['axo']),
      session.model ?? '',
      session.gitBranch ?? ''
    ]
      .join('\n')
      .toLowerCase()
  }

  function validDate(value: string | null): string | null {
    return value !== null && !Number.isNaN(Date.parse(value)) ? value : null
  }
</script>

{@render section('History', rows.length, collapsed, () => (collapsed = !collapsed))}
{#if !collapsed}
  <div class="px-2.5 py-1.5">
    <SearchInput
      bind:value={query}
      placeholder="Search sessions…"
      aria-label="Search session history"
      class="h-7 py-1 text-sm"
      spellcheck={false}
      oninput={() => (limit = PAGE)}
    />
  </div>

  {#if index.error}
    {@render statusLine('danger', index.error)}
  {/if}
  {#each issues as source (`${source.source.kind}:${source.source.kind === 'harness' ? `${source.source.harnessId}:${source.source.preset ?? ''}` : ''}`)}
    {@render statusLine(
      source.status === 'error' ? 'danger' : 'waiting',
      `${source.source.kind === 'axo' ? 'axo' : launchLabel(sessionBrand(folderPath, source.source.harnessId).label, source.source.preset)}: ${source.message ?? 'Session listing failed'}`
    )}
  {/each}
  {#if cached}
    <SidebarRow variant="info" divider={false}>
      <HistoryIcon size={11} class="shrink-0 text-subtle" />
      <span>Showing cached history</span>
    </SidebarRow>
  {/if}

  {#if rows.length === 0}
    <SidebarRow variant="info" divider={false}>
      {loadingHistory ? 'Loading session history…' : 'No past sessions in this folder'}
    </SidebarRow>
  {:else if matches.length === 0}
    <SidebarRow variant="info" divider={false}>No matching sessions</SidebarRow>
  {:else}
    <div class="border-t border-edge/30 bg-surface py-1">
      {#each visible as session (`${session.source.kind}:${session.source.kind === 'harness' ? session.source.harnessId : ''}:${session.sessionId}`)}
        {@const brand = session.source.kind === 'harness' ? sessionBrand(folderPath, session.source.harnessId) : null}
        {@const preset = sessionLaunchPreset(folderPath, session)}
        {@const date = validDate(session.updatedAt)}
        {@const tooltip = [session.sessionId, session.model, session.gitBranch, date ? formatFullDate(date) : null]
          .filter((part) => part !== null)
          .join('\n')}
        <ContextMenu.Root>
          <ContextMenu.Trigger class="contents">
            <SidebarRow
              variant="leaf"
              on="surface"
              divider={false}
              class="h-auto min-h-8 py-1.5"
              title={tooltip}
              onclick={() => openIndexedSession(folderPath, session, preset)}
            >
              <span class="min-w-0 flex-1 space-y-0.5">
                <span class="block truncate text-fg"
                  >{session.title || (session.source.kind === 'axo' ? 'Untitled Chat' : 'Untitled Session')}</span
                >
                <span class="flex min-w-0 items-center gap-1 text-2xs text-subtle">
                  {#if session.source.kind === 'axo'}
                    <MessagesSquareIcon size={11} />
                  {:else if brand?.icon}
                    <HarnessIcon icon={brand.icon} size={11} />
                  {/if}
                  <span class="min-w-0 flex-1 truncate">
                    {brand ? launchLabel(brand.label, preset) : 'axo'}{session.model ? ` · ${session.model}` : ''}
                  </span>
                  {#if date}<span class="shrink-0 font-mono">{timeAgo(date)}</span>{/if}
                </span>
              </span>
            </SidebarRow>
          </ContextMenu.Trigger>
          {#if session.source.kind === 'harness' && brand}
            <ContextMenu.Content>
              <ContextMenu.Sub>
                <ContextMenu.SubTrigger>Open With</ContextMenu.SubTrigger>
                <ContextMenu.SubContent>
                  {#each session.source.presets as choice (choice ?? '')}
                    <ContextMenu.Item onSelect={() => openIndexedSession(folderPath, session, choice)}>
                      {#if brand.icon}<HarnessIcon icon={brand.icon} size={14} />{/if}
                      <span>{launchLabel(brand.label, choice)}</span>
                    </ContextMenu.Item>
                  {/each}
                </ContextMenu.SubContent>
              </ContextMenu.Sub>
            </ContextMenu.Content>
          {/if}
        </ContextMenu.Root>
      {/each}
      {#if matches.length > visible.length}
        <SidebarRow variant="action" on="surface" divider={false} onclick={() => (limit += PAGE)}>Show More</SidebarRow>
      {/if}
    </div>
  {/if}
{/if}

{#snippet statusLine(tone: StatusTone, message: string)}
  <SidebarRow variant="info" divider={false} class="h-auto min-h-6 items-start py-1">
    <StatusDot {tone} size="sm" class="mt-1 shrink-0" />
    <span class="min-w-0 flex-1 break-words">{message}</span>
  </SidebarRow>
{/snippet}
