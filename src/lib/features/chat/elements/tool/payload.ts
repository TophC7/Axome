import { parseDisplayDiff, patchRows, type DiffResult } from '$lib/features/chat/lineDiff'

export const record = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {}

export const integer = (value: unknown): value is number =>
  typeof value === 'number' && Number.isSafeInteger(value) && value >= 0

const encoder = new TextEncoder()
export const pathText = (value: unknown): value is string =>
  typeof value === 'string' && value.length > 0 && !/[\u0000-\u001f\u007f]/.test(value)

export type Truncation = Record<string, unknown> & { outputLines: number; outputBytes: number }

/** axo's truncation record: the fields every capped tool output shares; null when any disagrees. */
export function truncation(value: unknown): Truncation | null {
  const cut = record(value)
  return cut.truncated === true &&
    integer(cut.outputLines) &&
    integer(cut.outputBytes) &&
    integer(cut.totalLines) &&
    integer(cut.totalBytes) &&
    cut.totalLines >= cut.outputLines &&
    cut.totalBytes >= cut.outputBytes &&
    cut.maxBytes === 51200 &&
    cut.outputBytes <= 51200 &&
    cut.lastLinePartial === false
    ? (cut as Truncation)
    : null
}

/** Exact bytes and lines prove that stripping a footer cannot consume recorded output. */
export const coversBody = (cut: Truncation, body: string): boolean =>
  encoder.encode(body).length === cut.outputBytes && (body === '' ? 0 : body.split('\n').length) === cut.outputLines

/** axo's authoritative display diff wins (including empty); the unified patch is the fallback. */
export function editDiff(details: unknown): DiffResult | null {
  const fields = record(details)
  return typeof fields.diff === 'string'
    ? parseDisplayDiff(fields.diff)
    : typeof fields.patch === 'string'
      ? patchRows(fields.patch)
      : null
}
