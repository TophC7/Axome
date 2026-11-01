import { logicalKey } from '../../utils/keyboardEvent'

export type ChatKeyAction =
  'stop' | 'cycleThinking' | 'openModel' | 'cycleExpansion' | 'pageUp' | 'pageDown' | 'focusComposer' | null

export function resolveChatKey(
  event: KeyboardEvent,
  ctx: { inComposer: boolean; inEditable: boolean; running: boolean }
): ChatKeyAction {
  if (event.defaultPrevented || event.isComposing || event.ctrlKey || event.metaKey) return null
  const key = logicalKey(event)
  if (event.altKey) {
    if (event.shiftKey) return null
    switch (key.toLowerCase()) {
      case 'm':
        return 'openModel'
      case 'o':
        return event.repeat ? null : 'cycleExpansion'
      default:
        return null
    }
  }
  if (ctx.inComposer) {
    if (key === 'Tab' && event.shiftKey) return 'cycleThinking'
    if (event.shiftKey) return null
    if (key === 'Escape' && ctx.running) return 'stop'
    if (key === 'PageUp') return 'pageUp'
    if (key === 'PageDown') return 'pageDown'
    return null
  }
  return !ctx.inEditable && key.length === 1 ? 'focusComposer' : null
}
