import { backend } from '$lib/api/backend'
import { getFormatterPolicy } from '$lib/features/extensions/catalog.svelte'
import { formatDocumentWithLsp, getLspDocumentContext } from '$lib/features/editor/lsp/registry'
import { modelHostPath } from '$lib/features/editor/renderers/monaco/text/modelCache'
import { getSettings, loadSettings } from '$lib/features/settings/state/settings.svelte'
import type { LanguageFormatterPolicy, FormattingSettings } from '$lib/types/backend'

type Monaco = typeof import('monaco-editor')
type MonacoModel = import('monaco-editor').editor.ITextModel
type MonacoTextEdit = import('monaco-editor').languages.TextEdit

const registeredLanguages = new Set<string>()

let initialized = false

export function ensureFormattingProviders(monaco: Monaco, languageIds: string[]): void {
  if (!initialized) {
    initialized = true
    ensureFormatterSettingsCacheInvalidation()
    if (!getSettings()) void loadSettings()
  }

  for (const languageId of languageIds) {
    if (registeredLanguages.has(languageId)) continue
    registeredLanguages.add(languageId)
    monaco.languages.registerDocumentFormattingEditProvider(languageId, {
      provideDocumentFormattingEdits
    })
  }
}

async function provideDocumentFormattingEdits(model: MonacoModel): Promise<MonacoTextEdit[]> {
  const context = getLspDocumentContext(model)
  if (!context) return formatDocumentWithLsp(model)

  const policy = getFormatterPolicy(context.folderPath, model.getLanguageId())
  const formatter = policy ? await resolveFormatterSelection(policy, context.folderPath) : 'lsp'
  if (formatter === 'disabled') return []
  if (formatter === 'lsp') {
    return formatDocumentWithLsp(model)
  }

  try {
    const filePath = modelHostPath(model.uri)
    if (!filePath) return []
    const formatted = await backend.formatting.format(context.folderPath, filePath, formatter, model.getValue())
    return toFullDocumentEdit(model, formatted)
  } catch (error) {
    console.warn(`Formatter ${formatter} failed`, error)
  }

  return []
}

const formatterSettingsByFolder = new Map<string, Promise<FormattingSettings>>()
let formatterSettingsInvalidationStarted = false
let formatterSettingsCachingEnabled = true

async function resolveFormatterSelection(policy: LanguageFormatterPolicy, folderPath: string): Promise<string> {
  let selection: string | undefined
  try {
    const formatting = await resolveProjectFormattingSettings(folderPath)
    selection = formatting[policy.language_id]?.formatter
  } catch (error) {
    console.warn('Failed to load project-effective formatter settings', error)
    selection = getSettings()?.formatting[policy.language_id]?.formatter
  }
  return selection && policy.options.some((option) => option.id === selection) ? selection : policy.default
}

function ensureFormatterSettingsCacheInvalidation(): void {
  if (formatterSettingsInvalidationStarted) return
  formatterSettingsInvalidationStarted = true
  backend.settings
    .onChanged(() => {
      formatterSettingsByFolder.clear()
    })
    .catch(() => {
      formatterSettingsCachingEnabled = false
      formatterSettingsByFolder.clear()
    })
}

async function resolveProjectFormattingSettings(folderPath: string): Promise<FormattingSettings> {
  if (!formatterSettingsCachingEnabled) {
    const effective = await backend.settings.getEffective(folderPath)
    return effective.settings.formatting
  }

  let cached = formatterSettingsByFolder.get(folderPath)
  if (!cached) {
    cached = backend.settings
      .getEffective(folderPath)
      .then((effective) => effective.settings.formatting)
      .catch((error) => {
        formatterSettingsByFolder.delete(folderPath)
        throw error
      })
    formatterSettingsByFolder.set(folderPath, cached)
  }
  return cached
}

function toFullDocumentEdit(model: MonacoModel, formatted: string): MonacoTextEdit[] {
  if (formatted === model.getValue()) return []
  return [{ range: model.getFullModelRange(), text: formatted }]
}
