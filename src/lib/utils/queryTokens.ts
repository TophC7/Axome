// Whitespace-separated filter DSL tokens shared by sidebar searches. The
// query string stays the single source of truth for what's visible; filter
// menus toggle literal tokens in it instead of owning parallel state.

export function splitTokens(query: string): string[] {
  return query.split(/\s+/).filter(Boolean)
}

export function hasToken(query: string, token: string): boolean {
  const lower = token.toLowerCase()
  return splitTokens(query).some((t) => t.toLowerCase() === lower)
}

export function toggleToken(query: string, token: string): string {
  const tokens = splitTokens(query)
  const lower = token.toLowerCase()
  const idx = tokens.findIndex((t) => t.toLowerCase() === lower)
  if (idx >= 0) tokens.splice(idx, 1)
  else tokens.push(token)
  return tokens.join(' ')
}
