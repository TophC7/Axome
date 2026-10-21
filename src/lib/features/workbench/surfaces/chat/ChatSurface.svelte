<script lang="ts">
  import { Button, IconButton } from '$lib/components/ui/button'
  import { StatusDot, type StatusTone } from '$lib/components/ui/status-dot'
  import { ToggleChip } from '$lib/components/ui/toggle-chip'
  import { InfoTooltip } from '$lib/components/ui/tooltip'
  import * as chats from '$lib/features/chat/registry'
  import { transcriptBlocks } from '$lib/features/chat/transcriptBlocks'
  import * as Conversation from '$lib/features/chat/elements/conversation'
  import * as Message from '$lib/features/chat/elements/message'
  import { Reasoning } from '$lib/features/chat/elements/reasoning'
  import { Plan } from '$lib/features/chat/elements/plan'
  import * as Tool from '$lib/features/chat/elements/tool'
  import { toolKindVerb } from '$lib/features/chat/elements/tool/kinds'
  import { ElicitationForm } from '$lib/features/chat/elements/elicitation'
  import * as PromptInput from '$lib/features/chat/elements/prompt-input'
  import { ConfigPicker } from '$lib/features/chat/elements/model-picker'
  import { ContextUsage } from '$lib/features/chat/elements/context'
  import { notify } from '$lib/features/notifications/state.svelte'
  import { getHarness } from '$lib/features/sessions/harnesses/catalog'
  import { switchSessionView } from '$lib/features/sessions/service.svelte'
  import type { ChatSessionTab } from '$lib/features/workbench/model'
  import { restartTabRun } from '$lib/features/workbench/runLifecycle'
  import { isTabTransferring } from '$lib/features/workbench/transferService.svelte'
  import { openLink } from '$lib/features/workbench/links/openLink'
  import { MessageSquare, PaperclipIcon, TerminalIcon } from '$lib/icons/lucideExports'
  import type { ChatEntry, ChatRequest } from '$lib/types/backend'
  import { getErrorMessage } from '$lib/utils/client-error'

  const START_DELAY_MS = 300
  let { tab }: { tab: ChatSessionTab } = $props()
  const controller = $derived(chats.getOrCreate(tab))
  const s = $derived(controller.state)
  const label = $derived(getHarness(tab.folderPath, tab.harnessId)?.label ?? tab.harnessId)
  const locked = $derived(tab.locked)
  const disabled = $derived(locked || tab.status !== 'running')
  const ended = $derived(tab.status === 'exited' || tab.status === 'failed')
  const blocks = $derived(transcriptBlocks(s.entries))
  const lastEntryId = $derived(s.entries.at(-1)?.id)
  const canSend = $derived(
    s.turn === 'idle' && !disabled && (controller.draft.text.trim() !== '' || controller.draft.images.length > 0)
  )
  let conversation: { scrollToBottom(): void } | undefined = $state()
  let fileInput: HTMLInputElement | undefined = $state()

  $effect(() => {
    const current = controller
    const id = tab.id
    if (tab.status !== 'dormant' || isTabTransferring(id)) return
    // Avoid starting every dormant chat when quickly flicking through tabs.
    const timer = setTimeout(() => {
      if (tab.id === id && tab.status === 'dormant' && !isTabTransferring(id)) {
        void current.start().catch(() => {}) // The controller owns the start error strip.
      }
    }, START_DELAY_MS)
    return () => clearTimeout(timer)
  })

  function permissionFor(entryId: string) {
    return s.requests.find(
      (request): request is Extract<ChatRequest, { kind: 'permission' }> =>
        request.kind === 'permission' && `tool:${request.tool_call_id}` === entryId
    )
  }

  function toolGroup(entries: ChatEntry[]) {
    const counts = new Map<string, number>()
    let count = 0
    let tone: StatusTone = 'success'
    let awaiting = false
    for (const entry of entries) {
      if (entry.kind !== 'tool') continue
      count++
      counts.set(entry.tool_kind, (counts.get(entry.tool_kind) ?? 0) + 1)
      if (entry.status === 'failed') tone = 'danger'
      else if (tone !== 'danger' && (entry.status === 'pending' || entry.status === 'in_progress')) tone = 'working'
      if (permissionFor(entry.id)) awaiting = true
    }
    return {
      count,
      tone,
      awaiting,
      summary: Array.from(counts, ([kind, n]) => `${toolKindVerb(kind)} ${n}`).join(' · ')
    }
  }

  function onSubmit() {
    if (canSend) void controller.send().then(() => conversation?.scrollToBottom())
  }

  function onAddImages(files: File[]) {
    if (disabled) return
    if (!s.accepts_images) {
      notify.warning('Images not supported', `${label} does not accept images.`)
      return
    }
    const current = controller
    for (const file of files) {
      if (!file.type.startsWith('image/')) continue
      if (file.size > 10 * 1024 * 1024) {
        notify.warning('Image too large', file.name)
        continue
      }
      const reader = new FileReader()
      reader.onload = () => {
        if (typeof reader.result !== 'string') return
        current.draft.images.push({
          id: crypto.randomUUID(),
          name: file.name,
          mimeType: file.type,
          data: reader.result.slice(reader.result.indexOf(',') + 1)
        })
      }
      reader.onerror = () => notify.error('Read image failed', file.name)
      reader.readAsDataURL(file)
    }
  }

  async function restart() {
    try {
      await restartTabRun(tab)
    } catch (error) {
      controller.error = getErrorMessage(error)
    }
  }
</script>

{#snippet renderEntry(entry: ChatEntry)}
  {@const streaming = entry.id === lastEntryId && s.turn === 'running'}
  {#if entry.kind === 'user'}
    <Message.User content={entry.content} />
  {:else if entry.kind === 'message'}
    <div class="group">
      <Message.Response text={entry.text} {streaming} folderPath={tab.folderPath} />
      {#if !streaming}<Message.Actions text={entry.text} />{/if}
    </div>
  {:else if entry.kind === 'thought'}
    <Reasoning
      text={entry.text}
      {streaming}
      seconds={controller.thoughtSeconds.get(entry.id)}
      open={controller.disclosure.get(entry.id) ?? streaming}
      onOpenChange={(open) => controller.disclosure.set(entry.id, open)}
    />
  {:else if entry.kind === 'tool'}
    {@const permission = permissionFor(entry.id)}
    <Tool.Row
      title={entry.title}
      tool_kind={entry.tool_kind}
      status={entry.status}
      content={entry.content}
      locations={entry.locations}
      folderPath={tab.folderPath}
      awaiting={!!permission}
      open={controller.disclosure.get(entry.id) ?? (entry.status === 'failed' || !!permission)}
      onOpenChange={(open) => controller.disclosure.set(entry.id, open)}
      onOpenLocation={(path, line) => void openLink(line ? `${path}:${line}` : path, tab.folderPath)}
    >
      {#if permission}
        <Tool.Confirmation
          title={permission.title === entry.title ? undefined : permission.title}
          options={permission.options}
          onRespond={(id) => void controller.respond(permission.request_id, { kind: 'permission', option_id: id })}
        />
      {/if}
    </Tool.Row>
  {:else if entry.kind === 'plan'}
    <Plan
      items={entry.items}
      open={controller.disclosure.get(entry.id) ?? entry.items.some((item) => item.status !== 'completed')}
      onOpenChange={(open) => controller.disclosure.set(entry.id, open)}
    />
  {/if}
{/snippet}

<div data-slot="chat-surface" class="relative flex min-h-0 flex-1 flex-col bg-ground">
  {#if s.turn === 'reconnecting'}
    <div
      role="status"
      class="pointer-events-none absolute top-2 right-2 z-20 flex items-center gap-1.5 rounded border border-edge bg-raised px-2 py-1 text-xs text-muted"
    >
      <StatusDot tone="waiting" size="sm" />
      Reconnecting…
    </div>
  {/if}
  {#if controller.error}
    <div class="border-b border-danger-border bg-danger-bg px-2.5 py-1.5 text-base text-danger">{controller.error}</div>
  {/if}
  {#if ended}
    <div
      class="flex items-center justify-between gap-2 border-b border-edge bg-raised px-2.5 py-1.5 text-base text-muted"
    >
      <span>{tab.status === 'failed' ? 'Session failed to start.' : 'Session ended.'}</span>
      <Button size="xs" disabled={locked || isTabTransferring(tab.id)} onclick={restart}>Restart</Button>
    </div>
  {/if}

  <Conversation.Root bind:this={conversation}>
    {#if s.entries.length === 0}
      {#if s.turn === 'starting'}
        <div role="status" class="flex items-center justify-center gap-2 text-sm text-muted">
          <StatusDot tone="working" />
          Starting {label}…
        </div>
      {:else}
        <Conversation.EmptyState title={label} description="Send a message to start. Type / for commands.">
          {#snippet icon()}<MessageSquare size={28} />{/snippet}
        </Conversation.EmptyState>
      {/if}
    {/if}
    {#each blocks as block, index (block.kind === 'tools' ? block.id : block.entry.id)}
      {#if block.kind === 'tools'}
        {@const group = toolGroup(block.entries)}
        <Tool.Group
          count={group.count}
          summary={group.summary}
          tone={group.tone}
          open={controller.disclosure.get(block.id) ??
            ((index === blocks.length - 1 && s.turn === 'running') || group.awaiting)}
          onOpenChange={(open) => controller.disclosure.set(block.id, open)}
        >
          {#each block.entries as entry (entry.id)}{@render renderEntry(entry)}{/each}
        </Tool.Group>
      {:else}
        {@render renderEntry(block.entry)}
      {/if}
    {/each}
    {#each s.requests as request (request.request_id)}
      {#if request.kind === 'permission'}
        {#if !s.entries.some((entry) => entry.id === `tool:${request.tool_call_id}`)}
          <Tool.Confirmation
            title={request.title}
            options={request.options}
            onRespond={(id) => void controller.respond(request.request_id, { kind: 'permission', option_id: id })}
          />
        {/if}
      {:else if request.kind === 'elicitation'}
        <ElicitationForm
          message={request.message}
          schema={request.schema}
          onRespond={(response) =>
            void controller.respond(request.request_id, {
              kind: 'elicitation',
              action: response.action,
              content: response.content ?? null
            })}
        />
      {/if}
    {/each}
  </Conversation.Root>

  <PromptInput.Root
    bind:text={controller.draft.text}
    images={controller.draft.images}
    acceptsImages={s.accepts_images}
    commands={s.commands}
    {disabled}
    placeholder={`Message ${label}…`}
    {onSubmit}
    {onAddImages}
    onRemoveImage={(id) => {
      controller.draft.images = controller.draft.images.filter((image) => image.id !== id)
    }}
  >
    {#snippet toolbar()}
      <PromptInput.Toolbar>
        {#snippet start()}
          {#if s.accepts_images}
            <IconButton tooltip="Attach Image" {disabled} onclick={() => fileInput?.click()}>
              <PaperclipIcon size={14} />
            </IconButton>
            <input
              bind:this={fileInput}
              type="file"
              accept="image/*"
              multiple
              hidden
              {disabled}
              onchange={(event) => {
                onAddImages(Array.from(event.currentTarget.files ?? []))
                event.currentTarget.value = ''
              }}
            />
          {/if}
          {#each s.config_options as option (option.id)}
            {#if option.kind.type === 'select'}
              <ConfigPicker
                {option}
                disabled={locked || s.turn !== 'idle'}
                onSelect={(value) => void controller.setConfig(option.id, { type: 'select', value })}
              />
            {:else if option.kind.type === 'boolean'}
              <ToggleChip
                pressed={option.kind.current_value}
                disabled={locked || s.turn !== 'idle'}
                class="font-sans text-xs"
                onclick={() =>
                  void controller.setConfig(option.id, { type: 'boolean', value: !option.kind.current_value })}
                >{option.name}</ToggleChip
              >
            {/if}
          {/each}
        {/snippet}
        {#snippet end()}
          <ContextUsage usage={s.usage} />
          <IconButton tooltip="Switch to Terminal" disabled={locked} onclick={() => void switchSessionView(tab.id)}>
            <TerminalIcon size={14} />
          </IconButton>
          <InfoTooltip ariaLabel="About Chat">
            Chat runs this harness over ACP. Compared with the terminal: MCP servers come only from Sworm
            (sessions.mcp_servers and extensions), extension widgets are inert, ask prompts appear as forms, and login
            is unavailable. Switch to Terminal for the full TUI.
          </InfoTooltip>
          <PromptInput.Submit turn={s.turn} {canSend} onSend={onSubmit} onStop={() => void controller.cancel()} />
        {/snippet}
      </PromptInput.Toolbar>
    {/snippet}
  </PromptInput.Root>
</div>
