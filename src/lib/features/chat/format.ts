export function formatTokens(n: number): string {
  if (n < 1000) return String(n)
  if (n < 1e6) return `${(n / 1000).toFixed(1).replace(/\.0$/, '')}k`
  return `${(n / 1e6).toFixed(1).replace(/\.0$/, '')}M`
}

export function formatCost(usd: number): string {
  return `$${usd.toFixed(2)}`
}
