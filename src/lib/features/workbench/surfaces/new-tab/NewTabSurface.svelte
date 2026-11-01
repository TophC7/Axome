<script lang="ts">
  import FolderHero from '$lib/components/layout/FolderHero.svelte'
  import StageView from '$lib/components/layout/StageView.svelte'
  import { BlurFade } from '$lib/components/ui/blur-fade'
  import { Button } from '$lib/components/ui/button'
  import { Kbd, KbdGroup } from '$lib/components/ui/kbd'
  import { MagicCard } from '$lib/components/ui/magic-card'
  import { Separator } from '$lib/components/ui/separator'
  import { localHostLabel, openBrowser } from '$lib/features/browser/state.svelte'
  import { splitShortcut } from '$lib/features/command-palette/shortcuts/spec'
  import { getCommandShortcut } from '$lib/features/command-palette/shortcuts/registry.svelte'
  import { getGitSummary } from '$lib/features/git/state.svelte'
  import { getErrorMessage } from '$lib/utils/client-error'
  import { notify } from '$lib/features/notifications/state.svelte'
  import { getNixStatus } from '$lib/features/settings/state/nix.svelte'
  import { StatusDot, statusToneText } from '$lib/components/ui/status-dot'
  import { svgUrl, type HarnessIconSource } from '$lib/features/sessions/harnesses/catalog'
  import HarnessIcon from '$lib/features/sessions/harnesses/HarnessIcon.svelte'
  import { getLaunchOptions, getHarnessesLoading } from '$lib/features/sessions/harnesses/state.svelte'
  import { newChat, startSession } from '$lib/features/sessions/service.svelte'
  import { startShell } from '$lib/features/shells/service.svelte'
  import { getShellLaunchOptions } from '$lib/features/shells/state.svelte'
  import { openTaskTab } from '$lib/features/tasks/service.svelte'
  import { getTasksReactive } from '$lib/features/tasks/state.svelte'
  import { createUntitledTextSurface } from '$lib/features/workbench/surfaces/text/service.svelte'
  import LucideIcon from '$lib/icons/LucideIcon.svelte'
  import MaskIcon from '$lib/icons/MaskIcon.svelte'
  import nixosUrl from '$lib/assets/nixos.svg?url'
  import {
    ArrowDown,
    ArrowUp,
    ArrowUpRightIcon,
    FilePlusCornerIcon,
    GitBranchIcon,
    MessagesSquareIcon,
    MonitorIcon,
    Play,
    ServerIcon,
    TerminalIcon
  } from '$lib/icons/lucideExports'
  import { platform } from '$lib/platform'
  import type { TaskDefinition } from '$lib/types/backend'
  import { basename, splitRemotePath } from '$lib/utils/paths'

  let { folderPath }: { folderPath: string } = $props()

  let remote = $derived(splitRemotePath(folderPath))
  let git = $derived(getGitSummary(folderPath))
  // `changes` lists a path twice when it is both staged and unstaged.
  let changedCount = $derived(git ? new Set(git.changes.map((change) => change.path)).size : 0)
  let nix = $derived(getNixStatus(folderPath))
  let tasks = $derived(getTasksReactive(folderPath))
  let harnessesLoading = $derived(getHarnessesLoading())
  let options = $derived(getLaunchOptions(folderPath))
  let shellOptions = $derived(getShellLaunchOptions(folderPath))
  // Stagger slot after the direct row; New File always reserves one so tasks never jump.
  let tasksDelay = $derived(0.1 + (options.length + shellOptions.length + 3) * 0.08)

  interface CardMeta {
    label: string
    icon: HarnessIconSource
    gradientFrom: string
    gradientTo: string
    wordmarkUrl?: string
    wordmarkAspect?: number
    wordmarkFont?: string
  }

  const chat: CardMeta = {
    label: 'Chat',
    icon: MessagesSquareIcon,
    wordmarkFont: 'var(--font-sans)',
    gradientFrom: 'var(--color-accent)',
    gradientTo: 'var(--color-warm)'
  }
  const shell: CardMeta = {
    label: 'Terminal',
    icon: TerminalIcon,
    wordmarkFont: 'var(--font-mono)',
    gradientFrom: 'var(--color-muted)',
    gradientTo: 'var(--color-edge-strong)'
  }
  const newFile: CardMeta = {
    label: 'New File',
    icon: FilePlusCornerIcon,
    wordmarkFont: 'var(--font-mono)',
    gradientFrom: 'var(--color-muted)',
    gradientTo: 'var(--color-edge-strong)'
  }
  let openShortcut = $derived(splitShortcut(getCommandShortcut('open-folder')))

  function runTask(task: TaskDefinition) {
    void openTaskTab(folderPath, task).catch((error) => notify.error('Run task failed', getErrorMessage(error)))
  }
</script>

{#snippet card(option: CardMeta, delay: number, onselect: (() => void) | null, detail: string | null)}
  {@const enabled = onselect !== null}
  <BlurFade {delay}>
    <MagicCard
      class="w-full"
      gradientFrom={option.gradientFrom}
      gradientTo={option.gradientTo}
      disabled={!enabled}
      onclick={() => onselect?.()}
    >
      <div class="flex items-center gap-3 p-3">
        <HarnessIcon icon={option.icon} size={32} class="text-muted" />
        <div class="flex min-w-0 flex-col gap-0.5">
          {#if option.wordmarkUrl && option.wordmarkAspect}
            <MaskIcon
              src={option.wordmarkUrl}
              width={Math.round(14 * option.wordmarkAspect)}
              height={14}
              label={option.label}
              class="self-start text-fg transition-colors group-hover:text-bright"
            />
          {:else}
            <span
              class="truncate text-md leading-tight font-medium text-fg transition-colors group-hover:text-bright"
              style:font-family={option.wordmarkFont ?? 'inherit'}>{option.label}</span
            >
          {/if}
          {#if !enabled}
            <span class="text-2xs text-subtle italic">{harnessesLoading ? 'Detecting...' : 'Not detected'}</span>
          {:else if detail}
            <span class="truncate font-mono text-2xs text-subtle">{detail}</span>
          {/if}
        </div>
      </div>
    </MagicCard>
  </BlurFade>
{/snippet}

{#snippet divider(label: string, delay: number)}
  <BlurFade {delay}>
    <div class="my-4 flex items-center gap-4">
      <Separator class="flex-1" />
      <span class="text-sm text-muted">{label}</span>
      <Separator class="flex-1" />
    </div>
  </BlurFade>
{/snippet}

<StageView>
  {#snippet footer()}
    <Button variant="outline" size="sm" class="text-muted" onclick={() => openBrowser()}>
      <ArrowUpRightIcon size={14} class="shrink-0" />
      Open Another Project…
      <KbdGroup class="ml-1">
        {#each openShortcut as part (part)}<Kbd class="h-5 min-w-5 rounded-sm px-1">{part}</Kbd>{/each}
      </KbdGroup>
    </Button>
  {/snippet}

  <FolderHero {folderPath} class="mb-8">
    <div class="mt-3 flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-sm text-muted">
      <span class="inline-flex items-center gap-1.5">
        {#if remote}
          <ServerIcon size={12} class="shrink-0" /><span class="font-mono">{remote.server}</span>
        {:else}
          <MonitorIcon size={12} class="shrink-0" />{localHostLabel()}
        {/if}
      </span>
      {#if git?.is_repo}
        <span class="inline-flex items-center gap-1.5 font-mono">
          <GitBranchIcon size={12} class="shrink-0" />{git.branch ?? 'Detached'}
          {#if git.ahead}<span class="inline-flex items-center" title="Commits Ahead"
              ><ArrowUp size={12} />{git.ahead}</span
            >{/if}
          {#if git.behind}<span class="inline-flex items-center" title="Commits Behind"
              ><ArrowDown size={12} />{git.behind}</span
            >{/if}
        </span>
        <span class="inline-flex items-center gap-1.5">
          <StatusDot tone={changedCount ? 'waiting' : 'success'} size="sm" />
          {changedCount ? `${changedCount} Changed` : 'Clean'}
        </span>
      {/if}
      {#if nix}
        <span class="inline-flex items-center gap-1.5">
          <MaskIcon src={nixosUrl} width={12} label="Nix" class={statusToneText[nix.tone]} />{nix.text}
        </span>
      {/if}
    </div>
  </FolderHero>

  <!-- Agents -->
  <div class="grid grid-cols-2 gap-2.5">
    {@render card(chat, 0.1, () => newChat(folderPath), 'axo')}
    {#each options as { harness, preset, status }, i (`${harness.id}:${preset ?? ''}`)}
      {@const meta = {
        label: harness.label,
        icon: svgUrl(harness.icon),
        wordmarkUrl: harness.wordmark ? svgUrl(harness.wordmark) : undefined,
        wordmarkAspect: harness.wordmark_aspect ?? undefined,
        wordmarkFont: harness.wordmark_font ?? undefined,
        gradientFrom: harness.gradient[0],
        gradientTo: harness.gradient[1]
      }}
      {@render card(
        meta,
        0.1 + (i + 1) * 0.08,
        status?.status === 'connected' ? () => startSession(folderPath, harness.id, preset) : null,
        preset ?? status?.version ?? null
      )}
    {/each}
  </div>

  {@render divider('or', 0.1 + (options.length + 1) * 0.08)}

  <!-- Direct options -->
  <div class="grid grid-cols-2 gap-2.5">
    {#each shellOptions as option, i (option.preset)}
      {@render card(
        { ...shell, label: option.label },
        0.1 + (options.length + 2 + i) * 0.08,
        () => startShell(folderPath, option.preset),
        basename(option.shell)
      )}
    {/each}
    {#if platform.native}
      {@render card(
        newFile,
        0.1 + (options.length + 2 + shellOptions.length) * 0.08,
        () => createUntitledTextSurface(folderPath),
        null
      )}
    {/if}
  </div>

  {#if tasks.length > 0}
    {@render divider('tasks', tasksDelay)}
    <BlurFade delay={tasksDelay + 0.08}>
      <div class="flex flex-wrap justify-center gap-2">
        {#each tasks as task (task.id)}
          <Button size="sm" title={task.command} onclick={() => runTask(task)}>
            {#if task.icon}
              <LucideIcon name={task.icon} size={14} class="shrink-0" />
            {:else}
              <Play size={14} class="shrink-0" />
            {/if}
            {task.label}
          </Button>
        {/each}
      </div>
    </BlurFade>
  {/if}
</StageView>
