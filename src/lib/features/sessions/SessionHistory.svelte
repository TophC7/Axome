<!--
  @component
  SessionHistory — the Sessions view's lower pane: the folder's searchable
  agent session history, per-source status, and explicit harness resume
  presets in Open With.
-->

<script lang="ts">
  import { iconButtonVariants } from '$lib/components/ui/button'
  import * as ContextMenu from '$lib/components/ui/context-menu'
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu'
  import { SearchInput } from '$lib/components/ui/input'
  import { SidebarRow } from '$lib/components/ui/sidebar-row'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { Spinner } from '$lib/components/ui/progress'
  import { Filter, MessagesSquareIcon } from '$lib/icons/lucideExports'
  import type { IndexedSession } from '$lib/types/backend'
  import { cn } from '$lib/utils/cn'
  import { timeAgo } from '$lib/utils/date'
  import { logicalKey } from '$lib/utils/keyboardEvent'
  import { hasToken, toggleToken } from '$lib/utils/queryTokens'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import { launchLabel, sessionBrand } from '$lib/features/sessions/harnesses/catalog'
  import {
    matchesSession,
    modelKey,
    modelName,
    parseSessionQuery,
    sessionAgent,
    sortSessions,
    type SessionSort
  } from './query'
  import { openIndexedSession, sessionLaunchPreset } from './service.svelte'
  import { getSessionIndex } from './state.svelte'

  let { folderPath, rows }: { folderPath: string; rows: IndexedSession[] } = $props()

  const PAGE = 100
  const SORT_LABEL: Record<SessionSort, string> = {
    'updated:desc': 'Updated — newest first',
    'updated:asc': 'Updated — oldest first',
    'messages:desc': 'Messages — most first'
  }
  const SORT_MODES = Object.keys(SORT_LABEL) as SessionSort[]

  // `query` is the single source of truth for what's visible; the filter
  // menu toggles literal DSL tokens in it.
  let query = $state('')
  let sort = $state<SessionSort>('updated:desc')
  let limit = $state(PAGE)
  let modelSearch = $state('')
  let modelSearchInput = $state<HTMLInputElement | null>(null)

  let index = $derived(getSessionIndex(folderPath))
  let sources = $derived(index.snapshot?.sources ?? [])
  let refreshing = $derived(index.loading || sources.some((source) => source.refreshing))
  // Ready sources carry a message only when some entries couldn't be read.
  let issues = $derived(
    sources.filter((source) => source.status === 'error' || source.status === 'missing' || source.message !== null)
  )
  let loadingHistory = $derived(
    index.snapshot === null
      ? index.loading
      : rows.length === 0 && sources.some((source) => source.refreshing || source.status === 'pending')
  )
  let parsed = $derived(parseSessionQuery(query))
  let filtering = $derived(parsed.agents.length + parsed.models.length + parsed.texts.length > 0)
  let matches = $derived(
    sortSessions(filtering ? rows.filter((session) => matchesSession(folderPath, session, parsed)) : rows, sort)
  )
  let visible = $derived(matches.slice(0, limit))

  // Menu choices come from what this folder's history actually holds.
  let agentOptions = $derived.by(() => {
    const seen = new Map<string, IndexedSession['source']>()
    for (const session of rows) {
      const agent = sessionAgent(session)
      if (!seen.has(agent)) seen.set(agent, session.source)
    }
    return [...seen].map(([agent, source]) => ({
      token: `agent:${agent}`,
      brand: source.kind === 'harness' ? sessionBrand(folderPath, source.harnessId) : null
    }))
  })
  let modelOptions = $derived.by(() => {
    const seen = new Map<string, string>()
    for (const session of rows) {
      if (session.model && !seen.has(modelKey(session.model)))
        seen.set(modelKey(session.model), modelName(session.model))
    }
    return [...seen]
      .map(([key, label]) => ({ token: `model:${key}`, label }))
      .sort((a, b) => a.label.localeCompare(b.label))
  })
  let shownModels = $derived.by(() => {
    const needle = modelSearch.trim().toLowerCase()
    return needle ? modelOptions.filter((option) => option.label.toLowerCase().includes(needle)) : modelOptions
  })
  let selectedModels = $derived(parsed.models.length)

  function setQuery(next: string) {
    query = next
    limit = PAGE
  }

  // The menu treats keys as typeahead and arrow navigation; keep them in the
  // field, except Escape/Tab, and let ArrowDown step into the model list.
  function modelSearchKeydown(event: KeyboardEvent) {
    const key = logicalKey(event)
    if (key === 'Escape' || key === 'Tab') return
    event.stopPropagation()
    if (key !== 'ArrowDown') return
    event.preventDefault()
    modelSearchInput?.closest('[role="menu"]')?.querySelector<HTMLElement>('[role="menuitemcheckbox"]')?.focus()
  }

  function validDate(value: string | null): string | null {
    return value !== null && !Number.isNaN(Date.parse(value)) ? value : null
  }
</script>

<div class="flex h-full flex-col">
  <div class="flex shrink-0 items-center px-2.5 py-1.5">
    <span class="inline-flex items-center gap-1.5 text-xs font-semibold tracking-wide text-muted uppercase">
      <span>History</span>
      {#if rows.length > 0}<span class="font-mono text-2xs font-medium text-subtle">{rows.length}</span>{/if}
      {#if refreshing}
        <Spinner size={11} class="text-muted" label="Refreshing session history" />
      {/if}
    </span>
  </div>
  <div class="shrink-0 px-2.5 pb-1.5">
    <SearchInput
      bind:value={() => query, setQuery}
      placeholder="Search sessions…"
      aria-label="Search session history"
      class="h-7 py-1 pr-7 text-sm"
      spellcheck={false}
    >
      <DropdownMenu.Root>
        <DropdownMenu.Trigger
          class={cn(iconButtonVariants(), 'absolute top-1/2 right-0.5 size-6 -translate-y-1/2')}
          aria-label="Filter options"
        >
          <Filter size={12} />
        </DropdownMenu.Trigger>
        <DropdownMenu.Content class="min-w-[230px]" align="end">
          {#if agentOptions.length > 0}
            <DropdownMenu.Group>
              <DropdownMenu.GroupHeading>Agent</DropdownMenu.GroupHeading>
              {#each agentOptions as option (option.token)}
                <DropdownMenu.CheckboxItem
                  checked={hasToken(query, option.token)}
                  onCheckedChange={() => setQuery(toggleToken(query, option.token))}
                  closeOnSelect={false}
                >
                  {#if option.brand?.icon}
                    <HarnessIcon icon={option.brand.icon} size={14} />
                  {:else if !option.brand}
                    <MessagesSquareIcon size={14} />
                  {/if}
                  <span>{option.brand?.label ?? 'axo'}</span>
                  <span class="ms-auto font-mono text-2xs text-muted">{option.token}</span>
                </DropdownMenu.CheckboxItem>
              {/each}
            </DropdownMenu.Group>
            <DropdownMenu.Separator />
          {/if}

          {#if modelOptions.length > 0}
            <DropdownMenu.Sub onOpenChange={(open) => !open && (modelSearch = '')}>
              <DropdownMenu.SubTrigger>
                <span>Model</span>
                {#if selectedModels > 0}<DropdownMenu.Shortcut>{selectedModels}</DropdownMenu.Shortcut>{/if}
              </DropdownMenu.SubTrigger>
              <DropdownMenu.SubContent
                class="w-60"
                onOpenAutoFocus={(event) => {
                  event.preventDefault()
                  modelSearchInput?.focus()
                }}
              >
                <div class="px-1.5 pb-1">
                  <SearchInput
                    bind:value={modelSearch}
                    bind:ref={modelSearchInput}
                    placeholder="Filter models…"
                    aria-label="Filter models"
                    class="h-7 py-1 text-sm"
                    spellcheck={false}
                    onkeydown={modelSearchKeydown}
                  />
                </div>
                <div class="max-h-64 overflow-y-auto">
                  {#each shownModels as option (option.token)}
                    <DropdownMenu.CheckboxItem
                      checked={hasToken(query, option.token)}
                      onCheckedChange={() => setQuery(toggleToken(query, option.token))}
                      closeOnSelect={false}
                    >
                      <span class="truncate">{option.label}</span>
                    </DropdownMenu.CheckboxItem>
                  {:else}
                    <div class="px-3 py-1.5 text-muted">No matching models</div>
                  {/each}
                </div>
              </DropdownMenu.SubContent>
            </DropdownMenu.Sub>
            <DropdownMenu.Separator />
          {/if}

          <DropdownMenu.RadioGroup
            value={sort}
            onValueChange={(value) => {
              sort = value as SessionSort
              limit = PAGE
            }}
          >
            <DropdownMenu.GroupHeading>Sort</DropdownMenu.GroupHeading>
            {#each SORT_MODES as mode (mode)}
              <DropdownMenu.RadioItem value={mode} closeOnSelect={false}>{SORT_LABEL[mode]}</DropdownMenu.RadioItem>
            {/each}
          </DropdownMenu.RadioGroup>
        </DropdownMenu.Content>
      </DropdownMenu.Root>
    </SearchInput>
  </div>

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if index.error}
      {@render statusLine('danger', index.error)}
    {/if}
    {#each issues as source (`${source.source.kind}:${source.source.kind === 'harness' ? `${source.source.harnessId}:${source.source.preset ?? ''}` : ''}`)}
      {@render statusLine(
        source.status === 'error' ? 'danger' : source.status === 'missing' ? 'waiting' : 'idle',
        `${source.source.kind === 'axo' ? 'axo' : launchLabel(sessionBrand(folderPath, source.source.harnessId).label, source.source.preset)}: ${source.message ?? 'Session listing failed'}`
      )}
    {/each}

    {#if rows.length === 0}
      <SidebarRow variant="info" divider={false}>
        {loadingHistory ? 'Loading session history…' : 'No past sessions in this folder'}
      </SidebarRow>
    {:else if matches.length === 0}
      <SidebarRow variant="info" divider={false}>No matching sessions</SidebarRow>
    {:else}
      {#each visible as session (`${session.source.kind}:${session.source.kind === 'harness' ? session.source.harnessId : ''}:${session.sessionId}`)}
        {@const brand = session.source.kind === 'harness' ? sessionBrand(folderPath, session.source.harnessId) : null}
        {@const preset = sessionLaunchPreset(folderPath, session)}
        {@const date = validDate(session.updatedAt)}
        <ContextMenu.Root>
          <ContextMenu.Trigger class="contents">
            <SidebarRow
              variant="leaf"
              divider={false}
              class="h-auto py-1.5"
              onclick={() => openIndexedSession(folderPath, session, preset)}
            >
              <span class="min-w-0 flex-1 space-y-0.5">
                <span class="block truncate text-fg">{session.title || 'Untitled Session'}</span>
                {#if session.lastMessage}
                  <span class="block truncate text-xs text-muted">
                    <span class="text-subtle">{session.lastMessage.role === 'user' ? 'You:' : 'Agent:'}</span>
                    {session.lastMessage.text}
                  </span>
                {/if}
                <span class="flex min-w-0 items-center gap-1 text-2xs whitespace-nowrap text-subtle">
                  {#if session.source.kind === 'axo'}
                    <MessagesSquareIcon size={11} class="shrink-0" />
                  {:else if brand?.icon}
                    <HarnessIcon icon={brand.icon} size={11} class="shrink-0" />
                  {/if}
                  <span class="shrink-0">{brand ? launchLabel(brand.label, preset) : 'axo'}</span>
                  {#if session.messageCount}
                    <span class="shrink-0">· {session.messageCount} {session.messageCount === 1 ? 'msg' : 'msgs'}</span>
                  {/if}
                  {#if session.model}<span class="min-w-0 truncate">· {modelName(session.model)}</span>{/if}
                  {#if date}<span class="shrink-0">· <span class="font-mono">{timeAgo(date)}</span></span>{/if}
                </span>
              </span>
            </SidebarRow>
          </ContextMenu.Trigger>
          <!-- Only worth a menu when several presets of this harness share the session's store. -->
          {#if session.source.kind === 'harness' && brand && session.source.presets.length > 1}
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
        <SidebarRow variant="action" divider={false} onclick={() => (limit += PAGE)}>Show More</SidebarRow>
      {/if}
    {/if}
  </div>
</div>

{#snippet statusLine(tone: StatusTone, message: string)}
  <SidebarRow variant="info" divider={false} class="h-auto min-h-6 items-start py-1">
    <StatusDot {tone} size="sm" class="mt-1 shrink-0" />
    <span class="min-w-0 flex-1 break-words">{message}</span>
  </SidebarRow>
{/snippet}
