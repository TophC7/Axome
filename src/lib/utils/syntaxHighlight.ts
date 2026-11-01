import type { BundledLanguage, Highlighter, ThemedToken } from 'shiki'

export const SHIKI_THEME_NAME = 'sworm'
let highlighterPromise: Promise<Highlighter> | null = null

export function getHighlighter(): Promise<Highlighter> {
  return (highlighterPromise ??= import('shiki').then(
    ({ createHighlighter, createCssVariablesTheme, createJavaScriptRegexEngine }) =>
      createHighlighter({
        themes: [createCssVariablesTheme({ name: SHIKI_THEME_NAME, variablePrefix: '--shiki-' })],
        langs: [],
        // Match Monaco's engine; avoid the desktop runtime's crashing Oniguruma wasm path.
        engine: createJavaScriptRegexEngine({ forgiving: true })
      })
  ))
}

/** Tokenize with a lazily loaded bundled grammar; null when Shiki has none for `language`. */
export async function codeTokens(code: string, language: string): Promise<ThemedToken[][] | null> {
  const highlighter = await getHighlighter()
  // This lazy boundary keeps Shiki's grammar registry out of startup; only known languages load it.
  const { bundledLanguages, bundledLanguagesAlias } = await import('shiki')
  const grammar =
    bundledLanguages[language as keyof typeof bundledLanguages] ??
    bundledLanguagesAlias[language as keyof typeof bundledLanguagesAlias]
  if (!grammar) return null
  await highlighter.loadLanguage(grammar)
  return highlighter.codeToTokens(code, { lang: language as BundledLanguage, theme: SHIKI_THEME_NAME }).tokens
}
