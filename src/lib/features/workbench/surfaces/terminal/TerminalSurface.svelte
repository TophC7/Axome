<script lang="ts">
  import '@xterm/xterm/css/xterm.css'
  import { onMount, untrack } from 'svelte'
  import { Button } from '$lib/components/ui/button'
  import { isTerminalDropActive, terminalDropObserver } from '$lib/features/dnd/adapters/terminal.svelte'
  import type { TerminalTab } from '$lib/features/workbench/model'
  import { getErrorMessage } from '$lib/utils/client-error'
  import * as terminals from '$lib/features/terminal/registry'
  import type { TerminalManager } from '$lib/features/terminal/TerminalManager'
  import { restartTerminalProcess } from '$lib/features/terminal/lifecycle'
  import { getActiveTerminalTabId } from '$lib/features/workbench/state.svelte'
  import { isTabTransferring } from '$lib/features/workbench/transferService.svelte'
  import { isAnyModalOpen } from '$lib/utils/modalRegistry.svelte'

  const START_DELAY_MS = 300
  let { tab }: { tab: TerminalTab } = $props()
  let containerEl: HTMLDivElement | undefined = $state(undefined)
  let manager: TerminalManager | null = $state(null)
  let error = $state<string | null>(null)
  let reconnecting = $state(false)
  let ended = $derived(tab.status === 'exited' || tab.status === 'failed')
  let canAcceptDrop = $derived(!tab.locked && tab.status === 'running')
  let dropActive = $derived(canAcceptDrop && isTerminalDropActive(tab.id))
  let attachedKey: string | null = null
  let attachedTabId: string | null = null
  let attachGen = 0
  let cleanupErrorListener: (() => void) | null = null
  let cleanupReconnectListener: (() => void) | null = null

  function clearManagerListeners() {
    cleanupErrorListener?.()
    cleanupErrorListener = null
    cleanupReconnectListener?.()
    cleanupReconnectListener = null
    reconnecting = false
  }

  function focusIfCurrent(current: TerminalManager) {
    if (isAnyModalOpen() || current.tabId !== getActiveTerminalTabId()) return
    current.focus()
    requestAnimationFrame(() => {
      if (!isAnyModalOpen() && current.tabId === getActiveTerminalTabId()) current.focus()
    })
  }

  async function attachTerminal(key: string) {
    if (!containerEl) return
    const gen = ++attachGen
    if (attachedTabId) terminals.detach(attachedTabId)
    clearManagerListeners()
    manager = null
    const next = terminals.getOrCreate(tab)
    try {
      await next.attach(containerEl)
      if (gen !== attachGen) return
      attachedKey = key
      attachedTabId = tab.id
      manager = next
      next.setInputEnabled(!tab.locked && !isTabTransferring(tab.id))
      error = next.getLastError()
      cleanupErrorListener = next.registerErrorListener((message) => {
        error = message
      })
      cleanupReconnectListener = next.registerReconnectListener((value) => {
        reconnecting = value
      })
      focusIfCurrent(next)
    } catch (cause) {
      if (gen === attachGen) error = getErrorMessage(cause)
    }
  }

  async function startProcess(current: TerminalManager) {
    error = null
    try {
      await current.start()
      focusIfCurrent(current)
    } catch (cause) {
      if (manager === current) error = getErrorMessage(cause)
    }
  }

  // Task singleton reruns replace their manager; other runs retain the same terminal.
  $effect(() => {
    const key = tab.kind === 'task' ? `${tab.id}:${tab.runId}` : tab.id
    if (!containerEl || key === attachedKey) return
    untrack(() => void attachTerminal(key))
    return () => {
      attachGen++
    }
  })

  $effect(() => {
    const current = manager
    const id = tab.id
    if (!current || current.tabId !== id || ended || isTabTransferring(id)) return
    if (untrack(() => current.isPtyActive())) return
    if (current.behavior.startPolicy === 'immediate') {
      untrack(() => void startProcess(current))
      return
    }
    if (tab.status !== 'dormant') return
    // Avoid spawning every dormant run when quickly flicking through tabs.
    const timer = setTimeout(() => {
      if (tab.id === id && tab.status === 'dormant' && !isTabTransferring(id)) void startProcess(current)
    }, START_DELAY_MS)
    return () => clearTimeout(timer)
  })

  $effect(() => {
    manager?.setInputEnabled(!tab.locked && !isTabTransferring(tab.id))
  })

  onMount(() => () => {
    attachGen++
    if (attachedTabId) terminals.detach(attachedTabId)
    clearManagerListeners()
  })

  async function restart() {
    error = null
    try {
      await restartTerminalProcess(tab)
      if (manager) focusIfCurrent(manager)
    } catch (cause) {
      error = getErrorMessage(cause)
    }
  }
</script>

<div class="relative flex min-h-0 flex-1 flex-col bg-ground">
  {#if reconnecting}
    <div
      role="status"
      class="pointer-events-none absolute top-2 right-2 z-20 flex items-center gap-1.5 rounded border border-edge bg-raised px-2 py-1 text-xs text-muted"
    >
      <span class="h-1.5 w-1.5 rounded-full bg-warning"></span>
      Reconnecting…
    </div>
  {/if}
  {#if error}
    <div class="border-b border-danger-border bg-danger-bg px-2.5 py-1.5 text-base text-danger">{error}</div>
  {/if}
  {#if ended}
    <div
      class="flex items-center justify-between gap-2 border-b border-edge bg-raised px-2.5 py-1.5 text-base text-muted"
    >
      <span>{tab.status === 'failed' ? 'Process failed to start.' : 'Process exited.'}</span>
      <Button size="xs" onclick={restart}>Restart</Button>
    </div>
  {/if}
  <div
    class="relative min-h-0 flex-1"
    data-terminal-focus-scope
    bind:this={containerEl}
    {@attach terminalDropObserver({
      tabId: tab.id,
      folderPath: tab.folderPath,
      canAcceptDrop: () => canAcceptDrop,
      onInsertText: (text) => manager?.sendText(text)
    })}
  >
    {#if dropActive}
      <div class="pointer-events-none absolute inset-1 z-20 rounded-lg border border-accent/60 bg-accent/10">
        <div class="absolute inset-0 flex items-center justify-center">
          <span
            class="rounded border border-edge-strong/70 bg-raised/90 px-2 py-0.5 text-xs font-medium tracking-wide text-bright uppercase"
            >Insert Path</span
          >
        </div>
      </div>
    {/if}
  </div>
</div>
