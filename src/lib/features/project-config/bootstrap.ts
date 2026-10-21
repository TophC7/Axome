// Project-config bootstrap.
//
// Pulls every JSON schema declared by each host's Rust backend and pushes it
// into the Monaco schema registry, scoped by folderUri. Schemas load per host
// because settings schemas enumerate that host's extension ids. Matching config
// files get autocomplete, validation, and hover docs.
//
// Adding a new config schema is a backend-only change:
// <> define the type with `#[derive(JsonSchema)]`
// <> append an entry to `all_config_schemas()`
// <> no frontend change required

import { backend } from '$lib/api/backend'
import { registerSchema } from '$lib/features/editor/schemas/registry'
import { REMOTE_MODEL_SCHEME } from '$lib/features/editor/renderers/monaco/text/modelCache'
import { extensionHostKey } from '$lib/features/extensions/catalog.svelte'

const loads = new Map<string, Promise<void>>()

export function initProjectSchemas(folderPath?: string | null): Promise<void> {
  const host = extensionHostKey(folderPath)
  const existing = loads.get(host)
  if (existing) return existing
  const load = backend.configSchemas
    .list(folderPath)
    .then((entries) => {
      for (const entry of entries) {
        registerSchema({
          id: host ? `${entry.id}@${host}` : entry.id,
          fileMatch: entry.fileMatch,
          folderUri: host ? `${REMOTE_MODEL_SCHEME}://${host}/` : 'file:///',
          schema: entry.schema
        })
      }
    })
    .catch((error) => {
      loads.delete(host)
      throw error
    })
  loads.set(host, load)
  return load
}
