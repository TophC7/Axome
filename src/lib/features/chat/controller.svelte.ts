import { backend } from '$lib/api/backend'
import type { StreamHandle } from '$lib/api/transport'
import { notify } from '$lib/features/notifications/state.svelte'
import type { ChatTab, TabId } from '$lib/features/workbench/model'
import {
  addChatTab,
  getTabs,
  requestFocusTab,
  setChatTabSessionId,
  setChatTabStatus,
  setChatTabTitle
} from '$lib/features/workbench/state.svelte'
import type { ChatAnswer, ChatBlock, ChatEntry, ChatEvent, ChatState, ChatThinking } from '$lib/types/backend'
import { getErrorMessage } from '$lib/utils/client-error'
import { getOrCreate } from './registry'

export class ChatController {
  readonly tabId: TabId
  state = $state<ChatState | null>(null)
  connection = $state<'detached' | 'attaching' | 'live' | 'reconnecting' | 'closed'>('detached')
  error = $state<string | null>(null)
  draft = $state('')
  draftModel = $state<string | null>(null)
  draftThinking = $state<ChatThinking>('off')
  editing = $state<{ entryId: string; anchor: string | null; text: string } | null>(null)
  #handle: StreamHandle<void> | null = null
  #generation = 0

  constructor(tabId: TabId) {
    this.tabId = tabId
  }

  get #tab(): ChatTab | undefined {
    const tab = getTabs().find((candidate) => candidate.id === this.tabId)
    return tab?.kind === 'chat' ? tab : undefined
  }

  get busy(): boolean {
    return !!this.state && (this.state.live.running || this.state.live.maintenance !== null)
  }

  async attach(): Promise<void> {
    this.release()
    const tab = this.#tab
    if (!tab) return
    this.error = null
    if (!tab.sessionId) {
      this.state = null
      this.connection = 'live'
      setChatTabStatus(this.tabId, 'idle')
      return
    }

    this.connection = 'attaching'
    const generation = this.#generation
    try {
      const handle = backend.chat.attach(
        { attachId: crypto.randomUUID(), folderPath: tab.folderPath, sessionId: tab.sessionId },
        {
          onEvent: (event) => {
            if (generation === this.#generation) this.apply(event)
          }
        }
      )
      this.#handle = handle
      await handle.ready
    } catch (error) {
      if (generation !== this.#generation) return
      this.connection = 'closed'
      this.error = getErrorMessage(error)
      setChatTabStatus(this.tabId, 'failed')
      throw error
    }
  }

  apply(event: ChatEvent): void {
    switch (event.type) {
      case 'snapshot':
        this.state = event.state
        this.connection = 'live'
        this.error = null
        setChatTabTitle(this.tabId, event.state.live.title ?? 'New Chat')
        break
      case 'entry':
        if (this.state) {
          this.state.entries.push(event.entry)
          if (event.entry.body.kind === 'assistant') this.state.streaming = null
        }
        break
      case 'live':
        if (this.state) this.state.live = event.live
        setChatTabTitle(this.tabId, event.live.title ?? 'New Chat')
        break
      case 'delta': {
        if (!this.state) break
        const delta = event.delta
        if (delta.type === 'start') {
          this.state.streaming = []
        } else if (delta.type === 'toolOutput') {
          const tool = this.state.live.tools.find((candidate) => candidate.callId === delta.callId)
          if (tool) tool.output = delta.content
        } else {
          const blocks = this.state.streaming
          if (!blocks || delta.index > blocks.length) break
          if (delta.type === 'toolCall') {
            const block: ChatBlock = {
              type: 'toolCall',
              id: delta.id,
              name: delta.name,
              arguments: delta.arguments
            }
            if (delta.index === blocks.length) blocks.push(block)
            else blocks[delta.index] = block
          } else if (delta.index === blocks.length) {
            blocks.push(
              delta.type === 'text'
                ? { type: 'text', text: delta.text }
                : { type: 'thinking', text: delta.text, redacted: false }
            )
          } else {
            const block = blocks[delta.index]
            if (block.type === delta.type && (block.type === 'text' || block.type === 'thinking')) {
              block.text += delta.text
            }
          }
        }
        break
      }
      case 'notice':
        notify.error('axo', event.message)
        break
      case 'closed':
        if (event.missing) {
          setChatTabSessionId(this.tabId, null)
          this.state = null
          this.editing = null
          this.connection = 'live'
          this.error = null
          notify.info('Started a new chat', 'The previous axo session no longer exists.')
        } else {
          this.connection = 'closed'
          this.error = event.message
        }
        break
      case 'reconnecting':
        this.connection = 'reconnecting'
        break
    }
    setChatTabStatus(
      this.tabId,
      this.connection === 'closed'
        ? 'failed'
        : this.state?.live.questions.length
          ? 'waiting'
          : this.busy
            ? 'running'
            : 'idle'
    )
  }

  async send(): Promise<void> {
    const text = this.draft.trim()
    if (!text) return
    if (text === '/compact') {
      if (await this.compact()) this.draft = ''
      return
    }
    const tab = this.#tab
    if (!tab) return
    try {
      if (!tab.sessionId) {
        if (!this.draftModel) throw new Error('Select an axo model before sending a message.')
        const sessionId = await backend.chat.create(tab.folderPath, this.draftModel, this.draftThinking)
        setChatTabSessionId(this.tabId, sessionId)
        await this.attach()
        if (this.connection !== 'live' || this.#tab?.sessionId !== sessionId) {
          throw new Error(this.error ?? 'The axo session could not be attached.')
        }
      }
      const sessionId = this.#tab?.sessionId
      if (!sessionId) return
      if (this.state?.live.running) await backend.chat.steer(tab.folderPath, sessionId, text)
      else await backend.chat.prompt(tab.folderPath, sessionId, crypto.randomUUID(), text)
      if (this.draft.trim() === text) this.draft = ''
    } catch (error) {
      notify.error('axo', getErrorMessage(error))
    }
  }

  async #withSession(operation: (folderPath: string, sessionId: string) => Promise<unknown>): Promise<boolean> {
    const tab = this.#tab
    if (!tab?.sessionId) return false
    try {
      await operation(tab.folderPath, tab.sessionId)
      return true
    } catch (error) {
      notify.error('axo', getErrorMessage(error))
      return false
    }
  }

  async followUp(): Promise<void> {
    const text = this.draft.trim()
    if (!text) return
    if (await this.#withSession((folder, id) => backend.chat.followUp(folder, id, text))) {
      if (this.draft.trim() === text) this.draft = ''
    }
  }

  async stop(): Promise<void> {
    const tab = this.#tab
    // Closing a tab must observe abort failures and keep the tab open.
    if (tab?.sessionId) await backend.chat.abort(tab.folderPath, tab.sessionId)
  }

  async answer(callId: string, answers: ChatAnswer[]): Promise<void> {
    await this.#withSession((folder, id) => backend.chat.answer(folder, id, callId, answers))
  }

  async dismiss(callId: string): Promise<void> {
    await this.#withSession((folder, id) => backend.chat.dismiss(folder, id, callId))
  }

  async setModel(selector: string): Promise<void> {
    if (!this.#tab?.sessionId) this.draftModel = selector
    else await this.#withSession((folder, id) => backend.chat.setModel(folder, id, selector))
  }

  async setThinking(level: ChatThinking): Promise<void> {
    if (!this.#tab?.sessionId) this.draftThinking = level
    else await this.#withSession((folder, id) => backend.chat.setThinking(folder, id, level))
  }

  compact(): Promise<boolean> {
    return this.#withSession((folder, id) => backend.chat.compact(folder, id))
  }

  startEdit(entry: ChatEntry): void {
    if (entry.body.kind !== 'user') return
    this.editing = { entryId: entry.id, anchor: entry.body.anchor, text: entry.body.text }
  }

  cancelEdit(): void {
    this.editing = null
  }

  async submitEdit(): Promise<void> {
    const editing = this.editing
    if (!editing || !editing.text.trim()) return
    const succeeded = await this.#withSession(async (folder, id) => {
      await backend.chat.navigate(folder, id, editing.anchor)
      await backend.chat.prompt(folder, id, crypto.randomUUID(), editing.text)
    })
    if (succeeded && this.editing === editing) this.editing = null
  }

  async selectBranch(entryId: string): Promise<void> {
    await this.#withSession((folder, id) => backend.chat.selectBranch(folder, id, entryId))
  }

  async fork(entryId: string): Promise<void> {
    await this.#withSession(async (folderPath, sessionId) => {
      const fork = await backend.chat.fork(folderPath, sessionId, entryId)
      const tabId = addChatTab(folderPath, { sessionId: fork.sessionId, title: this.state?.live.title ?? 'New Chat' })
      const tab = getTabs().find((candidate) => candidate.id === tabId)
      if (tab?.kind === 'chat') getOrCreate(tab).draft = fork.editableText ?? ''
      requestFocusTab(tabId)
    })
  }

  release(): void {
    this.#generation++
    this.#handle?.dispose()
    this.#handle = null
    this.connection = 'detached'
  }
}
