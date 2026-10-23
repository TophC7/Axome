<script lang="ts">
  import { onMount } from 'svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { Button, IconButton } from '$lib/components/ui/button'
  import * as Collapsible from '$lib/components/ui/collapsible'
  import { Textarea } from '$lib/components/ui/input'
  import { StatusDot } from '$lib/components/ui/status-dot'
  import * as chats from '$lib/features/chat/registry'
  import { getChatModels, loadChatModels } from '$lib/features/chat/models.svelte'
  import { transcriptTurns, turnFoldOpen, turnSummary, type Step, type AssistantEntry } from '$lib/features/chat/turns'
  import * as Conversation from '$lib/features/chat/elements/conversation'
  import * as Message from '$lib/features/chat/elements/message'
  import { Reasoning } from '$lib/features/chat/elements/reasoning'
  import * as Tool from '$lib/features/chat/elements/tool'
  import * as Turn from '$lib/features/chat/elements/turn'
  import { AskCard } from '$lib/features/chat/elements/ask'
  import * as PromptInput from '$lib/features/chat/elements/prompt-input'
  import { ConfigPicker } from '$lib/features/chat/elements/model-picker'
  import { ContextUsage } from '$lib/features/chat/elements/context'
  import { notify } from '$lib/features/notifications/state.svelte'
  import type { ChatTab } from '$lib/features/workbench/model'
  import { openLink } from '$lib/features/workbench/links/openLink'
  import {
    ChevronLeft,
    ChevronRight,
    CopyIcon,
    GitForkIcon,
    MessagesSquareIcon,
    PencilIcon
  } from '$lib/icons/lucideExports'
  import type { ChatThinking } from '$lib/types/backend'
  import { getErrorMessage } from '$lib/utils/client-error'

  let { tab }: { tab: ChatTab } = $props()
  const controller = $derived(chats.getOrCreate(tab))
  const chat = $derived(controller.state)
  const models = $derived(getChatModels(tab.folderPath))
  const catalog = $derived(models.catalog)
  const turns = $derived(chat ? transcriptTurns(chat.entries, chat.streaming, chat.live) : [])
  const running = $derived(chat?.live.running ?? false)
  const connecting = $derived(controller.connection === 'attaching' || controller.connection === 'reconnecting')
  const model = $derived(chat?.live.model ?? controller.draftModel)
  const thinking = $derived(chat?.live.thinking ?? controller.draftThinking)
  const choices = $derived.by(() => {
    const choices = (catalog?.models ?? []).map((item) => ({
      value: item.selector,
      name: item.name,
      group: item.provider
    }))
    if (model && !choices.some((choice) => choice.value === model)) {
      choices.push({ value: model, name: model, group: model.split('/')[0] })
    }
    return choices
  })
  const levels = $derived(catalog?.models.find((item) => item.selector === model)?.thinking ?? [thinking])
  const disabled = $derived(controller.connection !== 'live' || !model || !!chat?.live.maintenance)
  const canSend = $derived(!disabled && controller.draft.trim().length > 0)
  const canBranch = $derived(controller.connection === 'live' && !controller.busy)
  const emptyDescription = $derived(
    models.error ??
      (catalog && !catalog.models.length
        ? 'No axo models are available on this host. Set `defaultModel` in ~/.axo/settings.jsonc and credentials in ~/.axo/auth.json or the environment.'
        : 'Ask anything about this folder.')
  )
  let disclosure = $state<Record<string, boolean>>({})
  let conversation = $state<{ scrollToBottom(): void }>()

  onMount(() => {
    if (controller.connection === 'detached') void reconnect()
    void loadChatModels(tab.folderPath).then(() => {
      const loaded = getChatModels(tab.folderPath).catalog
      if (!tab.sessionId && !controller.draftModel && loaded) {
        controller.draftModel = loaded.defaultModel ?? loaded.models[0]?.selector ?? null
        controller.draftThinking = loaded.defaultThinking
      }
    })
  })

  async function reconnect() {
    // attach stores the failure for the closed-connection alert.
    await controller.attach().catch(() => {})
  }

  function onSubmit() {
    if (canSend) void controller.send().then(() => conversation?.scrollToBottom())
  }

  function onAltSubmit() {
    if (!canSend) return
    if (running) void controller.followUp()
    else onSubmit()
  }

  async function stop() {
    try {
      await controller.stop()
    } catch (error) {
      notify.error('Stop chat failed', getErrorMessage(error))
    }
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text)
    } catch (error) {
      notify.error('Copy failed', getErrorMessage(error))
    }
  }

  function answerText(entry: AssistantEntry): string {
    return entry.body.blocks.flatMap((block) => (block.type === 'text' ? [block.text] : [])).join('\n\n')
  }

  function openLocation(path: string, line?: number) {
    void openLink(line ? `${path}:${line}` : path, tab.folderPath)
  }
</script>

{#snippet renderStep(step: Step, afterUser: boolean)}
  {#if step.kind === 'text'}
    <Message.Response text={step.text} streaming={step.entryId === null} folderPath={tab.folderPath} />
  {:else if step.kind === 'thinking'}
    <Reasoning
      text={step.redacted ? 'Thinking is redacted.' : step.text}
      streaming={step.entryId === null}
      open={disclosure[step.id] ?? step.entryId === null}
      onOpenChange={(open) => (disclosure[step.id] = open)}
    />
  {:else if step.kind === 'tool'}
    <Tool.Row
      call={step}
      folderPath={tab.folderPath}
      open={disclosure[step.id] ?? step.status === 'error'}
      onOpenChange={(open) => (disclosure[step.id] = open)}
      onOpenLocation={openLocation}
    />
  {:else if step.kind === 'notice'}
    <div class="text-sm whitespace-pre-wrap text-muted">{step.text}</div>
  {:else if step.kind === 'modelChange'}
    {#if afterUser}<div class="text-xs text-muted">Model changed to {step.model}</div>{/if}
  {:else if step.kind === 'thinkingChange'}
    {#if afterUser}<div class="text-xs text-muted">Thinking changed to {step.thinking}</div>{/if}
  {:else if step.kind === 'compaction' || step.kind === 'branchSummary'}
    <Collapsible.Root
      open={disclosure[step.id] ?? false}
      onOpenChange={(open) => (disclosure[step.id] = open)}
      class="border-y border-edge py-2"
    >
      <Collapsible.Trigger
        class="flex w-full items-center gap-2 text-left text-xs text-muted hover:text-bright focus-visible:shadow-focus-ring focus-visible:outline-none"
      >
        <ChevronRight size={12} class={disclosure[step.id] ? 'rotate-90' : ''} />
        <span>{step.kind === 'compaction' ? 'Conversation compacted' : 'Branch summary'}</span>
        {#if step.kind === 'compaction'}
          <span class="ml-auto font-mono text-subtle">{step.tokensBefore.toLocaleString()} tokens before</span>
        {/if}
      </Collapsible.Trigger>
      <Collapsible.Content class="pt-2">
        <Message.Response text={step.summary} streaming={false} folderPath={tab.folderPath} />
      </Collapsible.Content>
    </Collapsible.Root>
  {/if}
{/snippet}

<div data-slot="chat-surface" class="relative flex min-h-0 flex-1 flex-col bg-ground">
  {#if controller.connection === 'closed'}
    <Alert.Root variant="danger" layout="banner" class="flex items-center justify-between gap-2">
      <Alert.Description>{controller.error ?? 'The axo connection closed.'}</Alert.Description>
      <Button size="xs" onclick={reconnect}>Reconnect</Button>
    </Alert.Root>
  {:else if connecting}
    <div role="status" class="flex items-center gap-2 border-b border-edge px-3 py-1.5 text-xs text-muted">
      <StatusDot tone="waiting" size="sm" />
      {controller.connection === 'reconnecting' ? 'Reconnecting…' : 'Connecting…'}
    </div>
  {/if}

  <Conversation.Root bind:this={conversation}>
    {#if !chat?.entries.length}
      <Conversation.EmptyState title="axo" description={emptyDescription}>
        {#snippet icon()}<MessagesSquareIcon size={28} />{/snippet}
      </Conversation.EmptyState>
    {/if}
    {#each turns as turn (turn.id)}
      <div class="group/turn flex min-w-0 flex-col gap-3">
        {#if turn.user}
          {@const entry = turn.user}
          <div class="group/user flex min-w-0 flex-col gap-1">
            {#if controller.editing?.entryId === entry.id}
              <div class="flex flex-col gap-2">
                <Textarea
                  bind:value={controller.editing.text}
                  rows={3}
                  aria-label="Edit message"
                  disabled={!canBranch}
                />
                <div class="flex justify-end gap-2">
                  <Button size="sm" variant="ghost" onclick={() => controller.cancelEdit()}>Cancel</Button>
                  <Button
                    size="sm"
                    variant="accent"
                    disabled={!canBranch || !controller.editing.text.trim()}
                    onclick={() => void controller.submitEdit()}>Send</Button
                  >
                </div>
              </div>
            {:else}
              <Message.User text={entry.body.text} />
              <div class="flex items-center justify-end gap-1">
                {#if entry.body.siblings.length > 1}
                  {@const index = entry.body.siblings.indexOf(entry.id)}
                  <div class="mr-2 flex items-center gap-1 text-xs text-muted" aria-label="Message branches">
                    <IconButton
                      tooltip="Previous Branch"
                      disabled={!canBranch || index <= 0}
                      onclick={() => void controller.selectBranch(entry.body.siblings[index - 1])}
                      ><ChevronLeft size={12} /></IconButton
                    >
                    <span class="tabular-nums">{index + 1}/{entry.body.siblings.length}</span>
                    <IconButton
                      tooltip="Next Branch"
                      disabled={!canBranch || index >= entry.body.siblings.length - 1}
                      onclick={() => void controller.selectBranch(entry.body.siblings[index + 1])}
                      ><ChevronRight size={12} /></IconButton
                    >
                  </div>
                {/if}
                <div class="flex gap-1 opacity-0 group-hover/user:opacity-100 focus-within:opacity-100">
                  <IconButton tooltip="Edit" disabled={!canBranch} onclick={() => controller.startEdit(entry)}
                    ><PencilIcon size={14} /></IconButton
                  >
                  <IconButton tooltip="Fork" disabled={!canBranch} onclick={() => void controller.fork(entry.id)}
                    ><GitForkIcon size={14} /></IconButton
                  >
                  <IconButton tooltip="Copy" onclick={() => void copy(entry.body.text)}
                    ><CopyIcon size={14} /></IconButton
                  >
                </div>
              </div>
            {/if}
          </div>
        {/if}
        {#if turn.steps.length}
          <Turn.Fold
            summary={turnSummary(turn)}
            open={disclosure[`fold:${turn.id}`] ?? turnFoldOpen(turn)}
            onOpenChange={(open) => (disclosure[`fold:${turn.id}`] = open)}
          >
            {#each turn.steps as step (step.id)}{@render renderStep(step, turn.user !== null)}{/each}
          </Turn.Fold>
        {/if}
        {#if turn.answer}
          {@const answer = turn.answer}
          <div class="group/answer flex flex-col gap-1">
            <Message.Response text={answerText(answer)} streaming={false} folderPath={tab.folderPath} />
            <div class="flex gap-1 self-start opacity-0 group-hover/answer:opacity-100 focus-within:opacity-100">
              <IconButton tooltip="Fork" disabled={!canBranch} onclick={() => void controller.fork(answer.id)}
                ><GitForkIcon size={14} /></IconButton
              >
              <IconButton tooltip="Copy" onclick={() => void copy(answerText(answer))}
                ><CopyIcon size={14} /></IconButton
              >
            </div>
          </div>
        {/if}
        <Turn.Files steps={turn.steps} folderPath={tab.folderPath} onOpenLocation={openLocation} />
        <Turn.Notice {turn} />
        <Turn.Footer {turn} />
      </div>
    {/each}
    {#each chat?.live.questions ?? [] as questions (questions.callId)}
      <AskCard
        {questions}
        onAnswer={(callId, answers) => void controller.answer(callId, answers)}
        onDismiss={(callId) => void controller.dismiss(callId)}
      />
    {/each}
    {#if chat?.live.maintenance}
      <div role="status" class="flex items-center gap-2 text-sm text-muted">
        <StatusDot tone="working" size="sm" />
        {chat.live.maintenance === 'compact'
          ? 'Compacting…'
          : chat.live.maintenance === 'branchSummary'
            ? 'Summarizing branch…'
            : 'Updating context…'}
      </div>
    {/if}
    {#if chat?.live.retry}
      <div role="status" class="text-sm text-muted">Retrying (attempt {chat.live.retry.attempt})…</div>
    {/if}
  </Conversation.Root>

  {#if chat?.live.queue.length}
    <ul aria-label="Queued messages" class="mx-auto flex w-full max-w-3xl flex-col gap-1 px-4 py-2">
      {#each chat.live.queue as item}
        <li class="flex gap-2 text-xs text-muted">
          <span class="shrink-0 font-medium">{item.kind === 'steer' ? 'Steer' : 'Follow-up'}</span>
          <span class="min-w-0 break-words whitespace-pre-wrap">{item.text}</span>
        </li>
      {/each}
    </ul>
  {/if}
  <PromptInput.Root
    bind:text={controller.draft}
    {disabled}
    placeholder={running ? 'Steer axo…' : 'Ask axo…'}
    commands={chat?.commands ?? []}
    {onSubmit}
    {onAltSubmit}
  >
    {#snippet toolbar()}
      <PromptInput.Toolbar>
        {#snippet start()}
          <ConfigPicker
            label="Model"
            value={model ?? ''}
            {choices}
            disabled={controller.connection !== 'live' || controller.busy || !choices.length}
            onSelect={(value) => void controller.setModel(value)}
          />
          {#if levels.some((level) => level !== 'off')}
            <ConfigPicker
              label="Thinking"
              value={thinking}
              choices={levels.map((level) => ({ value: level, name: level }))}
              disabled={disabled || controller.busy}
              onSelect={(value) => void controller.setThinking(value as ChatThinking)}
            />
          {/if}
        {/snippet}
        {#snippet end()}
          {#if chat}<ContextUsage stats={chat.live.stats} />{/if}
          <PromptInput.Submit
            running={controller.busy}
            {connecting}
            {canSend}
            onSend={onSubmit}
            onStop={() => void stop()}
          />
        {/snippet}
      </PromptInput.Toolbar>
    {/snippet}
  </PromptInput.Root>
  {#if running}
    <div class="px-4 pb-2 text-center text-xs text-subtle">Enter steers · Alt+Enter queues a follow-up</div>
  {/if}
</div>
