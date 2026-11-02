// Filter DSL for the session history search.
//
// Free text matches a session's title, last message, id, agent, presets, model
// and branch. Operators match exactly:
//   agent:ID     harness id, or axo
//   model:NAME   model name as listed (claude-opus-5 must not pick claude-opus-5-5)
//
// Same-kind tokens combine with OR; cross-kind tokens AND together, as in
// the issues search. Menus toggle tokens via $lib/utils/queryTokens.

import { sessionBrand } from '$lib/features/sessions/harnesses/catalog'
import type { IndexedSession } from '$lib/types/backend'
import { splitTokens } from '$lib/utils/queryTokens'

/** A parsed search: same-kind values OR together, kinds AND. */
export interface SessionQuery {
  agents: string[]
  models: string[]
  texts: string[]
}

export type SessionSort = 'updated:desc' | 'updated:asc' | 'messages:desc'

export function parseSessionQuery(raw: string): SessionQuery {
  const query: SessionQuery = { agents: [], models: [], texts: [] }
  for (const tok of splitTokens(raw.toLowerCase())) {
    if (tok.startsWith('agent:') && tok.length > 6) query.agents.push(tok.slice(6))
    else if (tok.startsWith('model:') && tok.length > 6) query.models.push(tok.slice(6))
    else query.texts.push(tok)
  }
  return query
}

/** The `agent:` value for a session: its harness id, or `axo`. */
export function sessionAgent(session: IndexedSession): string {
  return session.source.kind === 'harness' ? session.source.harnessId.toLowerCase() : 'axo'
}

// "anthropic/claude-opus-5-5" → "claude-opus-5-5"; a trailing build date goes too.
export function modelName(model: string): string {
  return model.slice(model.lastIndexOf('/') + 1).replace(/-\d{8}$/, '')
}

/** The `model:` value for a model: its display name, lowercased, without whitespace so it stays one token. */
export function modelKey(model: string): string {
  return modelName(model).toLowerCase().replace(/\s+/g, '')
}

function searchText(folderPath: string, session: IndexedSession): string {
  return [
    session.title ?? '',
    session.lastMessage?.text ?? '',
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

export function matchesSession(folderPath: string, session: IndexedSession, query: SessionQuery): boolean {
  const { agents, models, texts } = query
  if (agents.length > 0 && !agents.includes(sessionAgent(session))) return false
  if (models.length > 0 && (session.model === null || !models.includes(modelKey(session.model)))) return false
  if (texts.length === 0) return true
  const text = searchText(folderPath, session)
  return texts.every((value) => text.includes(value))
}

export function sortSessions(sessions: IndexedSession[], sort: SessionSort): IndexedSession[] {
  // The index already lists sessions newest first.
  if (sort === 'updated:desc') return sessions
  if (sort === 'updated:asc') return sessions.toReversed()
  // Stable sort keeps newest first among equal counts; unknown counts sink.
  return sessions.toSorted((a, b) => (b.messageCount ?? -1) - (a.messageCount ?? -1))
}
