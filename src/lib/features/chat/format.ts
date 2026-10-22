import type { ChatCost } from '$lib/types/backend'

export function formatTokens(n: number): string {
  if (n < 1000) return String(n)
  if (n < 1e6) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`
  return `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M`
}

export function formatCost(cost: ChatCost): string {
  return `${cost.currency} ${cost.amount.toFixed(2)}`
}
