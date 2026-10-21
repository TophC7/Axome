// Monaco environment bootstrap. Sets up workers, theme, keybinding
// overrides, and per-host language support from extension catalogs.

import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker'
import CssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker'
import HtmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker'
import JsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker'
import TsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker'
import { initializeMonacoEditorServices } from '$lib/features/editor/renderers/monaco/core/editorServices'
import { ensureFolderLanguageSupport } from '$lib/features/editor/renderers/monaco/core/languageSupport'
import { attachMonacoKeybindingOverrides } from '$lib/features/editor/renderers/monaco/core/keybindings'
import { ensureMonacoLsp } from '$lib/features/editor/lsp/registry'
import { registerSwormTheme } from '$lib/features/editor/renderers/monaco/core/monacoTheme'
import { attachMonaco } from '$lib/features/editor/schemas/registry'

export { ensureFolderLanguageSupport }

self.MonacoEnvironment = {
  getWorker(_workerId: string, label: string): Worker {
    if (label === 'typescript' || label === 'javascript') return new TsWorker()
    if (label === 'json') return new JsonWorker()
    if (label === 'css' || label === 'less' || label === 'scss') return new CssWorker()
    if (label === 'html' || label === 'handlebars' || label === 'razor') return new HtmlWorker()
    return new EditorWorker()
  }
}

interface TsLanguageDefaults {
  modeConfiguration: {
    documentRangeFormattingEdits?: boolean
    onTypeFormattingEdits?: boolean
  }
  setDiagnosticsOptions(options: {
    noSemanticValidation: boolean
    noSyntaxValidation: boolean
    noSuggestionDiagnostics: boolean
  }): void
  setModeConfiguration(options: { documentRangeFormattingEdits?: boolean; onTypeFormattingEdits?: boolean }): void
}

interface CssLanguageDefaults {
  setDiagnosticsOptions(options: { validate?: boolean }): void
}

interface JsonLanguageDefaults {
  modeConfiguration: {
    documentFormattingEdits?: boolean
    documentRangeFormattingEdits?: boolean
  }
  setModeConfiguration(options: { documentFormattingEdits?: boolean; documentRangeFormattingEdits?: boolean }): void
}

const DISABLED_TS_DIAGNOSTICS = {
  noSemanticValidation: true,
  noSyntaxValidation: true,
  noSuggestionDiagnostics: true
} as const

function getTsLanguageService(monaco: typeof import('monaco-editor')): {
  javascriptDefaults: TsLanguageDefaults
  typescriptDefaults: TsLanguageDefaults
} {
  return monaco.languages.typescript as unknown as {
    javascriptDefaults: TsLanguageDefaults
    typescriptDefaults: TsLanguageDefaults
  }
}

function getCssLanguageServices(monaco: typeof import('monaco-editor')): CssLanguageDefaults[] {
  const cssLanguages = monaco.languages.css as unknown as {
    cssDefaults: CssLanguageDefaults
    scssDefaults: CssLanguageDefaults
    lessDefaults: CssLanguageDefaults
  }

  return [cssLanguages.cssDefaults, cssLanguages.scssDefaults, cssLanguages.lessDefaults]
}

function getJsonLanguageService(monaco: typeof import('monaco-editor')): JsonLanguageDefaults {
  return (monaco.languages.json as unknown as { jsonDefaults: JsonLanguageDefaults }).jsonDefaults
}

/** One-time Monaco setup: extension language support, theme, keybinding overrides. */
let initPromise: Promise<void> | null = null

export function initMonaco(monaco: typeof import('monaco-editor')): Promise<void> {
  if (initPromise) return initPromise

  initPromise = (async () => {
    initializeMonacoEditorServices()
    registerSwormTheme(monaco)
    // Monaco caches glyph metrics per font. A face that finishes loading after
    // the first measurement (bundled Monocraft, a newly chosen code font)
    // would otherwise keep the fallback's widths and misplace the cursor.
    document.fonts.addEventListener('loadingdone', () => monaco.editor.remeasureFonts())

    // Keep Monaco's built-in language features as fallback when an
    // external LSP is missing or disabled, but suppress TS diagnostics
    // because Monaco's standalone TS worker misreads Sworm's project
    // alias/runtime environment and produces false positives.
    const tsLanguageService = getTsLanguageService(monaco)
    tsLanguageService.javascriptDefaults.setDiagnosticsOptions(DISABLED_TS_DIAGNOSTICS)
    tsLanguageService.typescriptDefaults.setDiagnosticsOptions(DISABLED_TS_DIAGNOSTICS)
    tsLanguageService.javascriptDefaults.setModeConfiguration({
      ...tsLanguageService.javascriptDefaults.modeConfiguration,
      documentRangeFormattingEdits: false,
      onTypeFormattingEdits: false
    })
    tsLanguageService.typescriptDefaults.setModeConfiguration({
      ...tsLanguageService.typescriptDefaults.modeConfiguration,
      documentRangeFormattingEdits: false,
      onTypeFormattingEdits: false
    })

    const jsonLanguageService = getJsonLanguageService(monaco)
    jsonLanguageService.setModeConfiguration({
      ...jsonLanguageService.modeConfiguration,
      documentFormattingEdits: false,
      documentRangeFormattingEdits: false
    })

    // CSS diagnostics stay off even without an external LSP so Tailwind
    // at-rules like @source and @apply do not regress to false errors.
    for (const cssLanguageService of getCssLanguageServices(monaco)) {
      cssLanguageService.setDiagnosticsOptions({ validate: false })
    }

    await ensureMonacoLsp(monaco)
    await ensureFolderLanguageSupport(monaco, null).catch((error) =>
      console.warn('Extension language support failed', error)
    )

    const { KeyMod, KeyCode } = monaco
    monaco.editor.addKeybindingRules([
      { keybinding: KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyP, command: '-editor.action.quickCommand' },
      { keybinding: KeyCode.F1, command: '-editor.action.quickCommand' },
      { keybinding: KeyMod.CtrlCmd | KeyMod.Shift | KeyCode.KeyO, command: '-editor.action.quickOutline' }
    ])
    attachMonacoKeybindingOverrides(monaco)

    // Flush any JSON schemas that were registered before Monaco
    // finished loading (bootstrap runs before the first editor mount).
    attachMonaco(monaco)
  })().catch((err) => {
    // Allow retry on next editor mount if initialization fails.
    initPromise = null
    throw err
  })

  return initPromise
}
