export function formatTokens(n: number): string {
  if (n < 1000) return String(n)
  if (n < 1e6) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`
  return `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M`
}

export function formatCost(usd: number): string {
  return `$${usd.toFixed(2)}`
}

export function formatDuration(ms: number): string {
  if (ms < 1000) return `${Math.round(ms)}ms`
  if (ms < 60_000) return `${(ms / 1000).toFixed(1).replace(/\.0$/, '')}s`
  const seconds = Math.floor(ms / 1000)
  return `${Math.floor(seconds / 60)}m${seconds % 60}s`
}

export function formatCount(count: number, noun: string): string {
  return `${count} ${noun}${count === 1 ? '' : 's'}`
}
