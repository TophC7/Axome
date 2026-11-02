// Filter DSL for the issues sidebar.
//
// Free text matches id/title. Operators:
//   is:open|active|done|archived
//   p:N | p:N..M
//   epic:ID
//
// Same-kind tokens combine with OR (clicking is:open then is:done yields
// the union). Cross-kind tokens AND together. Token toggling lives in
// $lib/utils/queryTokens.

import type { Issue, IssueStatus } from '$lib/types/backend'
import { splitTokens } from '$lib/utils/queryTokens'
const TERMINAL: Partial<Record<IssueStatus, true>> = { completed: true, wont_fix: true, archived: true }

function isOpen(status: IssueStatus): boolean {
  return !TERMINAL[status]
}

export type Term =
  | { kind: 'is'; value: 'open' | 'active' | 'done' | 'archived' }
  | { kind: 'p'; min: number; max: number }
  | { kind: 'epic'; id: string }
  | { kind: 'text'; value: string }

export function parseQuery(raw: string): Term[] {
  const out: Term[] = []
  for (const tok of splitTokens(raw.toLowerCase())) {
    if (tok.startsWith('is:')) {
      const v = tok.slice(3)
      if (v === 'open' || v === 'active' || v === 'done' || v === 'archived') {
        out.push({ kind: 'is', value: v })
        continue
      }
    }
    if (tok.startsWith('p:')) {
      const m = tok.slice(2).match(/^(\d+)(?:\.\.(\d+))?$/)
      if (m) {
        const min = Number(m[1])
        const max = m[2] !== undefined ? Number(m[2]) : min
        out.push({ kind: 'p', min, max })
        continue
      }
    }
    if (tok.startsWith('epic:')) {
      out.push({ kind: 'epic', id: tok.slice(5) })
      continue
    }
    out.push({ kind: 'text', value: tok })
  }
  return out
}

function matchesIs(status: IssueStatus, value: 'open' | 'active' | 'done' | 'archived'): boolean {
  if (value === 'open') return isOpen(status)
  if (value === 'active') return status === 'in_progress'
  if (value === 'done') return status === 'completed'
  return status === 'archived'
}

export function matchesIssue(issue: Issue, terms: Term[]): boolean {
  for (const t of terms) {
    if (t.kind === 'text') {
      const hay = `${issue.id} ${issue.title}`.toLowerCase()
      if (!hay.includes(t.value)) return false
    } else if (t.kind === 'epic') {
      if (!issue.epicId || !issue.epicId.toLowerCase().includes(t.id)) return false
    }
  }
  const ps = terms.filter((t): t is Extract<Term, { kind: 'p' }> => t.kind === 'p')
  if (ps.length > 0 && !ps.some((t) => issue.priority >= t.min && issue.priority <= t.max)) return false
  const iss = terms.filter((t): t is Extract<Term, { kind: 'is' }> => t.kind === 'is')
  if (iss.length > 0 && !iss.some((t) => matchesIs(issue.status, t.value))) return false
  return true
}
