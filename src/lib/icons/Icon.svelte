<!--
  @component
  Icon — renders Hugeicons icon data (imported from `@hugeicons/core-free-icons`)
  as a 24×24 inline SVG painted in currentColor. Decorative unless `aria-label` is passed.
-->
<script lang="ts" module>
  /** Hugeicons icon data: `[tag, attrs]` children of a 24×24 SVG. Compatible with the package's IconSvgObject. */
  export type IconData = readonly (readonly [string, Readonly<Record<string, string | number>>])[]

  const kebab = (key: string) => key.replace(/([a-z0-9])([A-Z])/g, '$1-$2').toLowerCase()
</script>

<script lang="ts">
  import type { SVGAttributes } from 'svelte/elements'

  let {
    icon,
    size = 24,
    strokeWidth,
    class: className,
    ...rest
  }: SVGAttributes<SVGSVGElement> & { icon: IconData; size?: number; strokeWidth?: number; class?: string } = $props()

  // Icon data uses React-style camelCase keys plus a React `key`; SVG wants kebab-case attributes.
  const nodes = $derived(
    icon.map(([tag, attrs]) => {
      const out: Record<string, string | number> = {}
      for (const [k, v] of Object.entries(attrs)) if (k !== 'key') out[kebab(k)] = v
      if (strokeWidth !== undefined && 'stroke-width' in out) out['stroke-width'] = strokeWidth
      return [tag, out] as const
    })
  )
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  class={className}
  aria-hidden={rest['aria-label'] ? undefined : true}
  {...rest}
>
  {#each nodes as [tag, attrs], i (i)}
    <svelte:element this={tag} xmlns="http://www.w3.org/2000/svg" {...attrs} />
  {/each}
</svg>
