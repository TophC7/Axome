<script lang="ts">
  import { onMount } from 'svelte'
  import FolderHero from '$lib/components/layout/FolderHero.svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { BlurFade } from '$lib/components/ui/blur-fade'
  import { IconButton } from '$lib/components/ui/button'
  import { Particles } from '$lib/components/ui/particles'
  import * as chats from '$lib/features/chat/registry'
  import { getChatModels, loadChatModels } from '$lib/features/chat/models.svelte'
  import { resolveChatKey } from '$lib/features/chat/chatKeymap'
  import { transcriptTurns, turnDurationMs } from '$lib/features/chat/turns'
  import type { BlockView } from '$lib/features/chat/view'
  import { Conversation } from '$lib/features/chat/elements/conversation'
  import { UserBlock, AgentBlock } from '$lib/features/chat/elements/transcript'
  import { SessionBar } from '$lib/features/chat/elements/session-bar'
  import { PromptInput } from '$lib/features/chat/elements/prompt-input'
  import { ConfigPicker } from '$lib/features/chat/elements/model-picker'
  import { pickFileAsync } from '$lib/features/command-palette/state.svelte'
  import { notify } from '$lib/features/notifications/state.svelte'
  import type { ChatTab } from '$lib/features/workbench/model'
  import { openFileLocation } from '$lib/features/workbench/links/openLink'
  import { PaperclipIcon } from '$lib/icons/lucideExports'
  import type { ChatThinking } from '$lib/types/backend'
  import { getErrorMessage } from '$lib/utils/client-error'
  import { logicalKey } from '$lib/utils/keyboardEvent'

  let { tab }: { tab: ChatTab } = $props()
  const controller = $derived(chats.getOrCreate(tab))
  const chat = $derived(controller.state)
  const models = $derived(getChatModels(tab.folderPath))
  const catalog = $derived(models.catalog)
  const turns = $derived(chat ? transcriptTurns(chat.entries, chat.streaming, chat.live) : [])
  const running = $derived(chat?.live.running ?? false)
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
  const modelDisabled = $derived(controller.connection !== 'live' || controller.busy || !choices.length)
  const canSend = $derived(!disabled && controller.draft.trim().length > 0)
  const canBranch = $derived(controller.connection === 'live' && !controller.busy)
  const catalogMessage = $derived(
    models.error
      ? `${models.error} Check ~/.axo/settings.jsonc and credentials in ~/.axo/auth.json or the environment.`
      : catalog && !catalog.models.length
        ? 'No axo models are available on this host. Set defaultModel in ~/.axo/settings.jsonc and credentials in ~/.axo/auth.json or the environment.'
        : null
  )
  let expansion = $state<BlockView>('collapsed')
  let overrides = $state<Record<string, BlockView>>({})
  let conversation = $state<Conversation>()
  let composer = $state<PromptInput>()
  let modelOpen = $state(false)
  let now = $state(Date.now())
  const workingTurn = $derived(turns.find((turn) => turn.running))
  const elapsed = $derived(workingTurn ? turnDurationMs(workingTurn, now) : null)

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

  $effect(() => {
    if (!running) return
    now = Date.now()
    const timer = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(timer)
  })

  async function reconnect() {
    // attach stores the failure for the closed-connection status line.
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

  async function attachFile() {
    const path = await pickFileAsync(tab.folderPath, 'Attach a file…')
    // axo prompts are text-only; an @path reference lets the agent read the file itself.
    if (path !== null) composer?.focus(`@${path} `)
  }

  function onView(stepId: string, view: BlockView) {
    overrides[stepId] = view
  }

  function onKeyAction(event: KeyboardEvent) {
    const target = event.target instanceof HTMLElement ? event.target : null
    const inComposer = !!target?.closest('[data-slot="prompt-input-root"] textarea')
    const inEditable = !!target?.closest(
      'input, textarea, select, button, a, [contenteditable="true"], [role="textbox"]'
    )
    const action = resolveChatKey(event, { inComposer, inEditable, running })
    if (!action) return
    event.preventDefault()
    event.stopPropagation()
    switch (action) {
      case 'stop':
        void stop()
        break
      case 'cycleThinking':
        if (!disabled && !controller.busy && levels.length) {
          void controller.setThinking(levels[(levels.indexOf(thinking) + 1) % levels.length])
        }
        break
      case 'openModel':
        if (!modelDisabled) modelOpen = true
        break
      case 'cycleExpansion':
        expansion = expansion === 'collapsed' ? 'preview' : expansion === 'preview' ? 'full' : 'collapsed'
        overrides = {}
        break
      case 'pageUp':
        conversation?.scrollPage(-1)
        break
      case 'pageDown':
        conversation?.scrollPage(1)
        break
      case 'focusComposer':
        composer?.focus(logicalKey(event))
        break
    }
  }

  function openLocation(path: string, line?: number) {
    void openFileLocation(
      path,
      line === undefined ? undefined : { kind: 'position', lineNumber: line, column: 1 },
      tab.folderPath
    )
  }
</script>

<!-- Surface keymap (chatKeymap.ts) delegates from here; every control inside stays native. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  data-slot="chat-surface"
  role="region"
  aria-label="Axo agent"
  tabindex="-1"
  onkeydown={onKeyAction}
  class="relative flex min-h-0 flex-1 flex-col bg-ground"
>
  <SessionBar
    {controller}
    title={chat?.live.title ?? tab.title}
    sessionId={tab.sessionId}
    {elapsed}
    onReconnect={() => void reconnect()}
    onStop={() => void stop()}
  />
  <!-- isolate scopes the particles' negative z-index so they paint above this bg but under the transcript. -->
  <div class="relative isolate flex min-h-0 flex-1 flex-col">
    {#if !turns.length}<Particles class="-z-10 opacity-60" />{/if}
    <Conversation bind:this={conversation}>
      {#if !turns.length}
        <div class="flex flex-1 flex-col items-center justify-center gap-6 py-10">
          <FolderHero folderPath={tab.folderPath}>
            <p class="mt-3 text-sm text-muted">
              Ask anything, or type <span class="font-mono">/</span> for commands.
            </p>
          </FolderHero>
          {#if catalogMessage}
            <BlurFade delay={0.15} class="w-full max-w-xl">
              <Alert.Root variant={models.error ? 'danger' : 'info'}>
                <Alert.Description>{catalogMessage}</Alert.Description>
              </Alert.Root>
            </BlurFade>
          {/if}
        </div>
      {/if}
      {#each turns as turn (turn.id)}
        {#if turn.user}<UserBlock entry={turn.user} config={turn.config} {controller} {canBranch} />{/if}
        {#if turn.steps.length || turn.running || turn.end}
          <AgentBlock
            {turn}
            {controller}
            folderPath={tab.folderPath}
            {canBranch}
            now={turn.running ? now : 0}
            {expansion}
            {overrides}
            {onView}
            onOpenLocation={openLocation}
          />
        {/if}
      {/each}
      {#snippet footer()}
        {#if chat?.live.queue.length}
          <ul aria-label="Queued messages" class="flex flex-col gap-1 pb-2">
            {#each chat.live.queue as item}
              <li class="flex gap-2 text-xs text-subtle">
                <span class="shrink-0 font-mono">{item.kind === 'steer' ? 'steer:' : 'follow-up:'}</span>
                <span class="min-w-0 break-words whitespace-pre-wrap">{item.text}</span>
              </li>
            {/each}
          </ul>
        {/if}
        <PromptInput
          bind:this={composer}
          bind:text={controller.draft}
          {disabled}
          placeholder={running ? 'Steer axo…' : 'Ask axo…'}
          commands={chat?.commands ?? []}
          {onSubmit}
          {onAltSubmit}
        >
          {#snippet start()}
            <IconButton
              tooltip="Attach File"
              tooltipSide="top"
              size="md"
              class="hover:bg-raised"
              {disabled}
              onclick={() => void attachFile()}
            >
              <PaperclipIcon size={14} />
            </IconButton>
          {/snippet}
          {#snippet end()}
            <ConfigPicker
              label="Model"
              value={model ?? ''}
              {choices}
              bind:open={modelOpen}
              disabled={modelDisabled}
              onSelect={(value) => void controller.setModel(value)}
            />
            {#if levels.some((level) => level !== 'off')}
              <ConfigPicker
                label="Effort"
                value={thinking}
                choices={levels.map((level) => ({ value: level, name: level }))}
                disabled={disabled || controller.busy}
                onSelect={(value) => void controller.setThinking(value as ChatThinking)}
              />
            {/if}
          {/snippet}
        </PromptInput>
      {/snippet}
    </Conversation>
  </div>
</div>
