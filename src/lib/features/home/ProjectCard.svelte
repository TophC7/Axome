<script lang="ts">
  import ActivityHeatmap from '$lib/features/activity-map/ActivityHeatmap.svelte'
  import AheadBehindBadge from '$lib/features/git/AheadBehindBadge.svelte'
  import { sessionBrand } from '$lib/features/sessions/harnesses/catalog'
  import { GitBranchIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import appIconUrl from '$lib/assets/axome.svg?url'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import type { GitBrief } from '$lib/types/backend'
  import { cn } from '$lib/utils/cn'
  import { timeAgo } from '$lib/utils/date'
  import { parentPath } from '$lib/utils/paths'
  import type { HomeProject } from './projects'

  let {
    project,
    git,
    openCount,
    onOpen,
    onHighlight
  }: {
    project: HomeProject
    /** `undefined` while loading, `null` when the brief is unavailable. */
    git: GitBrief | null | undefined
    /** Number of other workbenches with this folder open. */
    openCount: number
    onOpen: () => void
    onHighlight: (active: boolean) => void
  } = $props()

  let provenance = $derived(
    [
      project.openedAt && `Opened in Axome ${timeAgo(project.openedAt)}`,
      project.agentAt && `Agent activity ${timeAgo(project.agentAt)}`
    ]
      .filter(Boolean)
      .join('\n')
  )
</script>

<button
  type="button"
  disabled={!project.exists}
  title={project.path}
  class="group flex min-w-0 cursor-pointer flex-col gap-2.5 rounded-lg border border-edge bg-surface p-3 text-left transition-colors hover:border-accent/40 focus-visible:shadow-focus-ring focus-visible:outline-none disabled:cursor-default disabled:opacity-40 disabled:grayscale"
  onclick={onOpen}
  onpointerenter={() => onHighlight(true)}
  onpointerleave={() => onHighlight(false)}
  onfocus={() => onHighlight(true)}
  onblur={() => onHighlight(false)}
>
  <div class="w-full min-w-0">
    <div class="flex items-baseline gap-2">
      <span class="truncate text-base font-medium text-fg transition-colors group-hover:text-bright">
        {project.name}
      </span>
      {#if openCount > 0}
        <span class="ml-auto shrink-0 text-2xs text-accent">Open in {openCount}</span>
      {/if}
    </div>
    <div class="truncate font-mono text-2xs text-subtle">{parentPath(project.path)}</div>
  </div>

  <div class="flex h-4 w-full min-w-0 items-center gap-1.5 text-2xs text-muted">
    {#if git?.is_repo}
      <Icon icon={GitBranchIcon} size={10} class="shrink-0 text-subtle" />
      <span class="truncate font-mono">{git.branch ?? 'detached'}</span>
      {#if git.changed > 0}
        <span class="shrink-0 text-warning">{git.changed} changed</span>
      {/if}
      <AheadBehindBadge ahead={git.ahead ?? 0} behind={git.behind ?? 0} twoColor />
    {/if}
  </div>

  <div class="w-full"><ActivityHeatmap counts={project.dailyCounts} /></div>

  <div class="flex w-full items-center gap-1">
    <img
      src={appIconUrl}
      alt={project.openedAt ? 'Opened in Axome' : ''}
      class={cn('size-3.5 shrink-0', !project.openedAt && 'opacity-50 grayscale')}
    />
    {#each project.harnesses as activity (activity.harness_id)}
      {@const brand = sessionBrand(null, activity.harness_id)}
      {#if brand.icon}
        <HarnessIcon icon={brand.icon} size={14} alt={brand.label} class="rounded-sm" />
      {/if}
    {/each}
    <span class="ml-auto shrink-0 text-2xs text-subtle" title={provenance}>{timeAgo(project.lastActive)}</span>
  </div>
</button>
