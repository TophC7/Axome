import type { IconProps } from '@lucide/svelte'
import type { Component } from 'svelte'
import { getExtensionCatalog } from '$lib/features/extensions/catalog.svelte'
import type { HarnessDescriptor } from '$lib/types/backend'

/** A brand mark's image URL, or a Lucide icon for generic options such as Terminal. */
export type HarnessIconSource = string | Component<IconProps>

export function getHarness(folderPath: string | null | undefined, id: string): HarnessDescriptor | null {
  return getExtensionCatalog(folderPath)?.runtime.harnesses.find((harness) => harness.id === id) ?? null
}

export function svgUrl(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`
}

/** Label + icon for a session tab's harness. Unknown ids show the id and no icon. */
export function sessionBrand(
  folderPath: string | null | undefined,
  harnessId: string
): { label: string; icon: HarnessIconSource | null } {
  const harness = getHarness(folderPath, harnessId)
  return { label: harness?.label ?? harnessId, icon: harness ? svgUrl(harness.icon) : null }
}

export function launchLabel(label: string, preset: string | null): string {
  return preset ? `${label} · ${preset}` : label
}
