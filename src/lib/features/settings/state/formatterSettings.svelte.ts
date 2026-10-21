import { backend } from '$lib/api/backend'
import type { FormatterStatus } from '$lib/types/backend'

let formatterStatuses = $state<FormatterStatus[]>([])
let loadGeneration = 0

export function getFormatterStatuses() {
  return formatterStatuses
}

export async function loadFormatterStatuses(folderPath?: string) {
  // The newest request owns the list: a slow response for a folder the user
  // has since left must not replace the active folder's formatters.
  const request = ++loadGeneration
  const statuses = await backend.formatting.listFormatters(folderPath)
  if (request !== loadGeneration) return
  formatterStatuses = statuses
}
