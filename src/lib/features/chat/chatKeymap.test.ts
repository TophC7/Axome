import { test } from 'node:test'
import assert from 'node:assert/strict'
import { resolveChatKey, type ChatKeyAction } from './chatKeymap'

test('surface keys respect composer, editable and component ownership', () => {
  type Context = Parameters<typeof resolveChatKey>[1]
  const composer: Context = { inComposer: true, inEditable: true, running: true }
  const transcript: Context = { ...composer, inComposer: false, inEditable: false }
  const cases: {
    name: string
    key: string
    modifiers?: Partial<KeyboardEvent>
    ctx?: Partial<Context>
    action: ChatKeyAction
  }[] = [
    { name: 'composer Enter belongs to send/steer', key: 'Enter', action: null },
    { name: 'composer Shift+Enter belongs to newline', key: 'Enter', modifiers: { shiftKey: true }, action: null },
    { name: 'composer Alt+Enter belongs to follow-up', key: 'Enter', modifiers: { altKey: true }, action: null },
    { name: 'Escape stops the running turn', key: 'Escape', action: 'stop' },
    { name: 'Escape while idle does nothing', key: 'Escape', ctx: { running: false }, action: null },
    { name: 'Shift+Tab cycles thinking', key: 'Tab', modifiers: { shiftKey: true }, action: 'cycleThinking' },
    {
      name: 'WebKit unidentified Shift+Tab cycles thinking',
      key: 'Unidentified',
      modifiers: { code: 'Tab', shiftKey: true },
      action: 'cycleThinking'
    },
    {
      name: 'WebKit unidentified Shift+Enter stays a newline',
      key: 'Unidentified',
      modifiers: { code: 'Enter', shiftKey: true },
      action: null
    },
    { name: 'Tab keeps normal focus traversal', key: 'Tab', action: null },
    { name: 'Alt+M opens model in composer', key: 'm', modifiers: { altKey: true }, action: 'openModel' },
    {
      name: 'Alt+M opens model in transcript',
      key: 'm',
      modifiers: { altKey: true },
      ctx: transcript,
      action: 'openModel'
    },
    { name: 'Alt+O cycles work details in composer', key: 'o', modifiers: { altKey: true }, action: 'cycleExpansion' },
    {
      name: 'Alt+O cycles work details in transcript',
      key: 'o',
      modifiers: { altKey: true },
      ctx: transcript,
      action: 'cycleExpansion'
    },
    {
      name: 'Alt+O cycles work details inside a control',
      key: 'o',
      modifiers: { altKey: true },
      ctx: { inComposer: false },
      action: 'cycleExpansion'
    },
    {
      name: 'Alt+O also cycles while idle',
      key: 'O',
      modifiers: { altKey: true },
      ctx: { running: false },
      action: 'cycleExpansion'
    },
    { name: 'plain o stays composer typing', key: 'o', action: null },
    { name: 'plain o focuses composer from transcript', key: 'o', ctx: transcript, action: 'focusComposer' },
    {
      name: 'Shift+Alt+O stays native',
      key: 'O',
      modifiers: { altKey: true, shiftKey: true },
      action: null
    },
    {
      name: 'Ctrl+Alt+O stays app-owned',
      key: 'o',
      modifiers: { altKey: true, ctrlKey: true },
      action: null
    },
    {
      name: 'Meta+Alt+O stays app-owned',
      key: 'o',
      modifiers: { altKey: true, metaKey: true },
      action: null
    },
    {
      name: 'held Alt+O does not cycle repeatedly in composer',
      key: 'o',
      modifiers: { altKey: true, repeat: true },
      action: null
    },
    {
      name: 'held Alt+O does not cycle repeatedly in transcript',
      key: 'o',
      modifiers: { altKey: true, repeat: true },
      ctx: transcript,
      action: null
    },
    {
      name: 'locally handled Alt+O stays handled',
      key: 'o',
      modifiers: { altKey: true, defaultPrevented: true },
      action: null
    },
    {
      name: 'locally handled Alt+O in transcript stays handled',
      key: 'o',
      modifiers: { altKey: true, defaultPrevented: true },
      ctx: transcript,
      action: null
    },
    {
      name: 'Alt+O during composition stays native',
      key: 'o',
      modifiers: { altKey: true, isComposing: true },
      action: null
    },
    { name: 'PageUp scrolls from composer', key: 'PageUp', action: 'pageUp' },
    { name: 'PageDown scrolls from composer', key: 'PageDown', action: 'pageDown' },
    { name: 'printable key focuses composer', key: 'a', ctx: transcript, action: 'focusComposer' },
    {
      name: 'shifted printable key focuses composer',
      key: 'A',
      modifiers: { shiftKey: true },
      ctx: transcript,
      action: 'focusComposer'
    },
    { name: 'space is printable', key: ' ', ctx: transcript, action: 'focusComposer' },
    { name: 'composer typing stays native', key: 'a', action: null },
    { name: 'inline editable typing stays native', key: 'a', ctx: { inComposer: false }, action: null },
    { name: 'ask Enter belongs to ask prompt', key: 'Enter', ctx: transcript, action: null },
    { name: 'ask Escape belongs to ask prompt', key: 'Escape', ctx: transcript, action: null },
    { name: 'inline edit Enter belongs to edit', key: 'Enter', ctx: { inComposer: false }, action: null },
    { name: 'inline edit Escape belongs to edit', key: 'Escape', ctx: { inComposer: false }, action: null },
    {
      name: 'ask number already handled locally',
      key: '1',
      modifiers: { defaultPrevented: true },
      ctx: transcript,
      action: null
    },
    { name: 'ask number inside input stays native', key: '9', ctx: { inComposer: false }, action: null },
    {
      name: 'Shift+Alt+M is not a surface binding',
      key: 'M',
      modifiers: { altKey: true, shiftKey: true },
      action: null
    },
    { name: 'Ctrl+O stays app-owned', key: 'o', modifiers: { ctrlKey: true }, ctx: transcript, action: null },
    { name: 'Ctrl+P stays app-owned', key: 'p', modifiers: { ctrlKey: true }, ctx: transcript, action: null },
    { name: 'Ctrl+T stays app-owned', key: 't', modifiers: { ctrlKey: true }, ctx: transcript, action: null },
    { name: 'IME composition is untouched', key: 'a', modifiers: { isComposing: true }, ctx: transcript, action: null },
    { name: 'handled events are untouched', key: 'PageUp', modifiers: { defaultPrevented: true }, action: null },
    { name: 'transcript PageUp remains native', key: 'PageUp', ctx: transcript, action: null },
    { name: 'transcript PageDown remains native', key: 'PageDown', ctx: transcript, action: null },
    {
      name: 'transcript Shift+Tab remains native',
      key: 'Tab',
      modifiers: { shiftKey: true },
      ctx: transcript,
      action: null
    },
    { name: 'Shift+PageUp remains native', key: 'PageUp', modifiers: { shiftKey: true }, action: null },
    {
      name: 'unknown Alt binding remains native',
      key: 'x',
      modifiers: { altKey: true },
      ctx: transcript,
      action: null
    },
    { name: 'non-printable key remains native', key: 'ArrowDown', ctx: transcript, action: null }
  ]
  for (const row of cases) {
    const event = {
      key: row.key,
      altKey: false,
      ctrlKey: false,
      metaKey: false,
      shiftKey: false,
      defaultPrevented: false,
      isComposing: false,
      repeat: false,
      ...row.modifiers
    } as KeyboardEvent
    assert.equal(resolveChatKey(event, { ...composer, ...row.ctx }), row.action, row.name)
  }
})
