<!--
  @component
  NamedIcon — renders a Hugeicons icon by kebab-case slug as shown in hugeicons.com URLs
  ("play", "search-01", "git-branch"). Used for names from user config (.axome/tasks.jsonc).
  Each icon loads as its own chunk; missing or unknown names render `fallback`.
-->
<script lang="ts" module>
  /** "search-01" → "Search01Icon" (the loader's key). */
  export function iconExportName(slug: string): string {
    return (
      slug
        .trim()
        .split('-')
        .filter(Boolean)
        .map((s) => s[0].toUpperCase() + s.slice(1))
        .join('') + 'Icon'
    )
  }
</script>

<script lang="ts">
  import { iconExists, loadIcon } from '@hugeicons/core-free-icons/loader'
  import Icon, { type IconData } from './Icon.svelte'

  let {
    name,
    fallback,
    size = 14,
    class: className
  }: { name: string | null | undefined; fallback: IconData; size?: number; class?: string } = $props()

  const key = $derived(name?.trim() ? iconExportName(name) : null)
  const known = $derived(key !== null && iconExists(key))
  let loaded = $state<{ key: string; icon: IconData } | null>(null)

  $effect(() => {
    if (!known || key === null) return
    const k = key
    let cancelled = false
    loadIcon(k)
      .then((icon) => {
        if (!cancelled) loaded = { key: k, icon }
      })
      .catch(() => {
        if (!cancelled) loaded = { key: k, icon: fallback }
      })
    return () => {
      cancelled = true
    }
  })

  // While a known icon is loading, render an empty SVG of the same size so the layout doesn't shift.
  const icon = $derived(!known ? fallback : loaded?.key === key ? loaded.icon : [])
</script>

<Icon {icon} {size} class={className} />
