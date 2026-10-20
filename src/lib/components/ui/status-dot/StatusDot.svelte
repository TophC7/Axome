<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants'

  export const statusDotVariants = tv({
    base: 'inline-block shrink-0 rounded-full',
    variants: {
      tone: {
        idle: 'bg-muted',
        working: 'bg-accent',
        waiting: 'bg-warning',
        success: 'bg-success',
        danger: 'bg-danger'
      },
      size: {
        sm: 'size-1.5',
        md: 'size-2',
        lg: 'size-2.5'
      },
      // Pinned to an icon's bottom-right. The ground-coloured border cuts the dot out of the
      // icon (ring-* is banned); with box-content, right-0.5/bottom-0.5 lands it exactly where
      // a right-1 bottom-1 ring-2 dot sat.
      corner: {
        true: 'pointer-events-none absolute right-0.5 bottom-0.5 box-content border-2 border-ground',
        false: ''
      }
    },
    defaultVariants: {
      tone: 'idle',
      size: 'md',
      corner: false
    }
  })

  export type StatusTone = NonNullable<VariantProps<typeof statusDotVariants>['tone']>
  export type StatusDotSize = NonNullable<VariantProps<typeof statusDotVariants>['size']>

  /** A state worth showing: its tone and a one-line description. */
  export interface ToneStatus {
    tone: StatusTone
    text: string
  }

  export const statusToneText: Record<StatusTone, string> = {
    idle: 'text-muted',
    working: 'text-accent',
    waiting: 'text-warning',
    success: 'text-success',
    danger: 'text-danger'
  }
</script>

<script lang="ts">
  import { cn } from '$lib/utils/cn'

  let {
    tone = 'idle',
    size = 'md',
    corner = false,
    label,
    class: className
  }: {
    tone?: StatusTone
    size?: StatusDotSize
    corner?: boolean
    label?: string
    class?: string
  } = $props()
</script>

{#if label}
  <span role="img" aria-label={label} class={cn(statusDotVariants({ tone, size, corner }), className)}></span>
{:else}
  <span aria-hidden="true" class={cn(statusDotVariants({ tone, size, corner }), className)}></span>
{/if}
