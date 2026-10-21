<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants'

  export const progressRingVariants = tv({
    base: 'shrink-0',
    variants: {
      tone: {
        muted: 'text-muted',
        warning: 'text-warning',
        danger: 'text-danger'
      }
    },
    defaultVariants: { tone: 'muted' }
  })

  export type ProgressRingTone = VariantProps<typeof progressRingVariants>['tone']

  const circumference = 2 * Math.PI * 9
</script>

<script lang="ts">
  import { cn } from '$lib/utils/cn'
  import type { SVGAttributes } from 'svelte/elements'

  let {
    value,
    size = 14,
    label,
    class: className,
    ...rest
  }: SVGAttributes<SVGSVGElement> & {
    value: number
    size?: number
    label?: string
    class?: string
  } = $props()

  let clampedValue = $derived(Math.min(1, Math.max(0, value)))
  let tone = $derived<ProgressRingTone>(clampedValue >= 0.95 ? 'danger' : clampedValue >= 0.8 ? 'warning' : 'muted')
</script>

<svg
  data-slot="progress-ring"
  viewBox="0 0 24 24"
  width={size}
  height={size}
  fill="none"
  stroke-width="3"
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  class={cn(progressRingVariants({ tone }), className)}
  {...rest}
>
  <circle cx="12" cy="12" r="9" class="text-edge" stroke="currentColor" />
  <circle
    cx="12"
    cy="12"
    r="9"
    stroke="currentColor"
    stroke-dasharray={circumference}
    stroke-dashoffset={circumference * (1 - clampedValue)}
    transform="rotate(-90 12 12)"
  />
</svg>
