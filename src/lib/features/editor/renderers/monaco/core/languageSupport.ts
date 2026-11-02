import type * as MonacoModule from 'monaco-editor'
import type { HighlighterCore, LanguageRegistration } from 'shiki/core'
import { backend } from '$lib/api/backend'
import { extensionHostKey, loadExtensionCatalog } from '$lib/features/extensions/catalog.svelte'
import { ensureLspLanguageProviders } from '$lib/features/editor/lsp/registry'
import { ensureFormattingProviders } from '$lib/features/editor/renderers/monaco/core/formatters'
import { registerNixLanguage } from '$lib/features/editor/renderers/monaco/core/languages/nix'
import { registerAxomeTheme, AXOME_SHIKI_THEME } from '$lib/features/editor/renderers/monaco/core/monacoTheme'
import { initProjectSchemas } from '$lib/features/project-config/bootstrap'
import type { ExtensionGrammar } from '$lib/types/backend'

type Monaco = typeof MonacoModule

const loads = new Map<string, Promise<void>>()
const grammars = new Map<string, ExtensionGrammar>()
let batchChain = Promise.resolve()
let highlighter: HighlighterCore | null = null
let grammarsChanged = false
let originalEditorMethods: Pick<Monaco['editor'], 'setTheme' | 'create'> | null = null

export function ensureFolderLanguageSupport(monaco: Monaco, folderPath: string | null): Promise<void> {
  const key = extensionHostKey(folderPath)
  const existing = loads.get(key)
  if (existing) return existing

  const load = batchChain
    .then(() => loadFolderLanguageSupport(monaco, folderPath))
    .catch((error) => {
      loads.delete(key)
      throw error
    })
  loads.set(key, load)
  batchChain = load.catch(() => {})
  return load
}

async function loadFolderLanguageSupport(monaco: Monaco, folderPath: string | null): Promise<void> {
  void initProjectSchemas(folderPath).catch((error) => console.warn('Config schema load failed', error))
  const [catalog, folderGrammars] = await Promise.all([
    loadExtensionCatalog(folderPath),
    backend.extensions.getGrammars(folderPath)
  ])
  const existing = new Set(monaco.languages.getLanguages().map((language) => language.id))
  for (const language of catalog.runtime.languages) {
    if (existing.has(language.id)) continue
    monaco.languages.register({ id: language.id, extensions: language.extensions, aliases: [language.label] })
    existing.add(language.id)
    if (language.id === 'nix') registerNixLanguage(monaco)
  }

  for (const grammar of folderGrammars) {
    // Monaco languages are window-global: the first host providing a grammar wins.
    if (grammars.has(grammar.language)) continue
    grammars.set(grammar.language, grammar)
    grammarsChanged = true
  }

  if (grammarsChanged) {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }, { shikiToMonaco }] = await Promise.all([
      import('shiki/core'),
      import('shiki/engine/javascript'),
      import('@shikijs/monaco')
    ])
    const next = await createHighlighterCore({
      themes: [AXOME_SHIKI_THEME],
      langs: [...grammars.values()].map(
        (grammar) =>
          ({
            ...grammar.grammar,
            name: grammar.language,
            embeddedLangs: grammar.embedded_languages
          }) as LanguageRegistration
      ),
      // Prefer the pure-JS engine here. The default Oniguruma wasm
      // path has been crashing inside Tauri/WebView during Monaco
      // background tokenization with out-of-bounds memory errors.
      engine: createJavaScriptRegexEngine({ forgiving: true })
    })

    originalEditorMethods ??= { setTheme: monaco.editor.setTheme, create: monaco.editor.create }
    // shikiToMonaco wraps both on every call; restoring them keeps exactly
    // one wrapper bound to the live highlighter.
    monaco.editor.setTheme = originalEditorMethods.setTheme
    monaco.editor.create = originalEditorMethods.create
    shikiToMonaco(next, monaco)
    registerAxomeTheme(monaco)
    const previous = highlighter
    highlighter = next
    previous?.dispose()
    // Keep this dirty on a failed rebuild so the next host attempt retries it.
    grammarsChanged = false
  }

  const ids = catalog.runtime.languages.map((language) => language.id)
  ensureLspLanguageProviders(monaco, ids)
  ensureFormattingProviders(monaco, ids)
}
