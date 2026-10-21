import { SvelteMap } from 'svelte/reactivity'
import { backend } from '$lib/api/backend'
import type { StreamHandle } from '$lib/api/transport'
import { notify } from '$lib/features/notifications/state.svelte'
import { isChatTab, type TabId } from '$lib/features/workbench/model'
import {
  clearSessionTabSessionId,
  getTabs,
  persistRunTabRunId,
  setRunTabRunId,
  setRunTabStatus,
  setSessionTabSessionId
} from '$lib/features/workbench/state.svelte'
import type {
  ChatConfigValue,
  ChatContent,
  ChatEvent,
  ChatResponse,
  ChatStartInfo,
  ChatState
} from '$lib/types/backend'
import { getErrorMessage } from '$lib/utils/client-error'

export interface ChatDraftImage {
  id: string
  name: string
  mimeType: string
  data: string
}

export function emptyChatState(): ChatState {
  return {
    session_id: null,
    turn: 'starting',
    entries: [],
    config_options: [],
    commands: [],
    usage: null,
    requests: [],
    accepts_images: false
  }
}

export class ChatController {
  readonly tabId: TabId
  state = $state<ChatState>(emptyChatState())
  error = $state<string | null>(null)
  draft = $state<{ text: string; images: ChatDraftImage[] }>({ text: '', images: [] })
  disclosure = new SvelteMap<string, boolean>()
  thoughtSeconds = new SvelteMap<string, number>()
  #handle: StreamHandle<ChatStartInfo> | null = null
  #runId: string | null = null
  #index = new Map<string, number>()
  #thought: { id: string; at: number } | null = null

  constructor(tabId: TabId) {
    this.tabId = tabId
  }

  #finishThought(): void {
    if (!this.#thought) return
    this.thoughtSeconds.set(this.#thought.id, Math.max(1, Math.round((performance.now() - this.#thought.at) / 1000)))
    this.#thought = null
  }

  apply(event: ChatEvent): void {
    switch (event.type) {
      case 'snapshot':
        this.state = event.state
        this.#index.clear()
        this.state.entries.forEach((entry, index) => this.#index.set(entry.id, index))
        this.thoughtSeconds.clear()
        this.#thought = null
        if (this.state.session_id !== null) setSessionTabSessionId(this.tabId, this.state.session_id)
        break
      case 'entry': {
        const entry = event.entry
        const index = this.#index.get(entry.id)
        if (this.#thought && this.#thought.id !== entry.id) this.#finishThought()
        if (index === undefined) {
          this.#index.set(entry.id, this.state.entries.length)
          this.state.entries.push(entry)
          if (entry.kind === 'thought' && this.state.turn === 'running') {
            this.#thought = { id: entry.id, at: performance.now() }
          }
        } else {
          this.state.entries[index] = entry
        }
        break
      }
      case 'textDelta': {
        const index = this.#index.get(event.id)
        const entry = index === undefined ? undefined : this.state.entries[index]
        if (entry?.kind === 'message' || entry?.kind === 'thought') entry.text += event.text
        break
      }
      case 'turnState':
        this.state.turn = event.turn
        if (event.turn !== 'running') this.#finishThought()
        break
      case 'configOptions':
        this.state.config_options = event.options
        break
      case 'commands':
        this.state.commands = event.commands
        break
      case 'usage':
        this.state.usage = event.usage
        break
      case 'request':
        this.state.requests.push(event.request)
        break
      case 'requestResolved':
        this.state.requests = this.state.requests.filter((request) => request.request_id !== event.request_id)
        break
      case 'sessionBound':
        this.state.session_id = event.session_id
        setSessionTabSessionId(this.tabId, event.session_id)
        break
      case 'notice':
        notify[event.level]('Chat', event.message)
        break
      case 'exit':
        setRunTabRunId(this.tabId, null)
        setRunTabStatus(this.tabId, 'exited')
        this.error = event.message
        this.#runId = null
        this.release()
        break
    }
  }

  async start(): Promise<void> {
    const tab = getTabs().find((candidate) => candidate.id === this.tabId)
    if (!tab || !isChatTab(tab)) return
    const runId = tab.runId ?? crypto.randomUUID()
    this.#runId = runId
    this.error = null
    this.state.turn = 'starting'
    setRunTabStatus(this.tabId, 'starting')
    try {
      await persistRunTabRunId(this.tabId, runId)
      // Capture before discovery can bind a different session id during startup.
      const supplied = tab.sessionId
      const handle = backend.chat.start(
        {
          runId,
          folderPath: tab.folderPath,
          harnessId: tab.harnessId,
          preset: tab.preset,
          sessionId: supplied
        },
        { onEvent: (event) => this.apply(event) }
      )
      this.#handle = handle
      const info = await handle.ready
      if (this.#handle !== handle) return
      setRunTabStatus(this.tabId, 'running')
      if (info.sessionId) setSessionTabSessionId(this.tabId, info.sessionId)
      else if (supplied !== null) {
        clearSessionTabSessionId(this.tabId, supplied)
        notify.info('Started a new conversation', 'The previous one no longer exists.')
      }
    } catch (error) {
      setRunTabStatus(this.tabId, 'failed')
      setRunTabRunId(this.tabId, null)
      this.error = getErrorMessage(error)
      this.#runId = null
      this.release()
      throw error
    }
  }

  async send(): Promise<void> {
    if (!this.#runId) return
    const content: ChatContent[] = this.draft.images.map((image) => ({
      type: 'image',
      mime_type: image.mimeType,
      data: image.data
    }))
    if (this.draft.text.trim()) content.push({ type: 'text', text: this.draft.text })
    if (content.length === 0) return
    try {
      await backend.chat.prompt(this.#runId, content)
      this.draft = { text: '', images: [] }
    } catch (error) {
      notify.error('Chat', getErrorMessage(error))
    }
  }

  async cancel(): Promise<void> {
    if (!this.#runId) return
    try {
      await backend.chat.cancel(this.#runId)
    } catch (error) {
      notify.error('Chat', getErrorMessage(error))
    }
  }

  async setConfig(id: string, value: ChatConfigValue): Promise<void> {
    if (!this.#runId) return
    try {
      await backend.chat.setConfig(this.#runId, id, value)
    } catch (error) {
      notify.error('Chat', getErrorMessage(error))
    }
  }

  async respond(requestId: string, response: ChatResponse): Promise<void> {
    if (!this.#runId) return
    try {
      await backend.chat.respond(this.#runId, requestId, response)
    } catch (error) {
      notify.error('Chat', getErrorMessage(error))
    }
  }

  async stop(): Promise<void> {
    if (this.#runId) await backend.chat.stop(this.#runId)
    this.release()
  }

  release(): void {
    this.#handle?.dispose()
    this.#handle = null
  }
}
