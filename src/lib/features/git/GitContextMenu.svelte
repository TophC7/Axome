<script lang="ts">
  import { backend } from '$lib/api/backend'
  import * as ContextMenu from '$lib/components/ui/context-menu'
  import {
    fetchFolder,
    forcePushWithLease,
    ignoreFiles,
    markResolved,
    popLatestStash,
    promptStash,
    publishBranch,
    pullFolder,
    pushFolder,
    stageAllFolder,
    takeConflictSide,
    syncFolder,
    undoLastCommit,
    unstageAllFolder
  } from '$lib/features/git/actions.svelte'
  import { pathGitState } from './gitStatus'
  import {
    canRevealInFileManager,
    confirmDiscardAll,
    confirmDiscardPaths,
    copyFullPatch,
    copyPatch,
    copyPath,
    copyRelativePath,
    describeTarget,
    openActualFile,
    openHeadFile,
    revealInExplorerView,
    revealInFileManager,
    runPathsAction,
    type GitTreeMenuTarget
  } from '$lib/features/git/treeActions'
  import { openWorkingTreeDiff } from '$lib/features/workbench/surfaces/diff/service.svelte'
  import {
    ArrowDown02Icon,
    ArrowDataTransferVerticalIcon,
    ArrowUp02Icon,
    ClipboardIcon,
    CheckIcon,
    CloudUploadIcon,
    EyeIcon,
    EyeOffIcon,
    FileDiffIcon,
    FolderOpenIcon,
    ListTreeIcon,
    CircleMinusIcon,
    PackageIcon,
    PackageOpenIcon,
    CirclePlusIcon,
    RefreshCwIcon,
    RefreshIcon,
    SquareArrowOutUpRightIcon,
    Delete02Icon,
    Undo02Icon
  } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import type { GitSummary } from '$lib/types/backend'
  import type { Snippet } from 'svelte'

  let {
    folderPath,
    summary,
    target,
    onResetTarget,
    children
  }: {
    folderPath: string
    summary: GitSummary
    target: GitTreeMenuTarget | null
    onResetTarget: () => void
    children: Snippet
  } = $props()

  let staged = $derived(target !== null && target.kind !== 'conflict' && target.section === 'staged')
  let remoteOk = $derived(canRevealInFileManager(folderPath))
  let trackedFiles = $derived.by(() => {
    if (!target || target.kind === 'file' || target.kind === 'conflict') return []
    const tracked = new Set(
      summary.changes.filter((change) => !change.staged && change.status !== '?').map((change) => change.path)
    )
    return target.files.filter((path) => tracked.has(path))
  })

  let resolving = $state(false)
  let extantConflictPath = $state<string | null>(null)

  $effect(() => {
    const conflict = target?.kind === 'conflict' ? target.conflict : null
    const folder = folderPath
    extantConflictPath = null
    if (!conflict || conflict.kind === 'both_deleted') return
    let cancelled = false
    void backend.files.stat(folder, conflict.path).then(
      (stat) => {
        if (!cancelled && stat.regular) extantConflictPath = conflict.path
      },
      () => {
        // Missing files and inaccessible paths cannot be revealed.
      }
    )
    return () => {
      cancelled = true
    }
  })

  async function resolveConflicts(files: string[], side?: 'ours' | 'theirs') {
    if (resolving) return
    resolving = true
    try {
      if (side) await takeConflictSide(folderPath, files, side)
      else await markResolved(folderPath, files)
    } finally {
      resolving = false
    }
  }
</script>

{#snippet revealEntries(path: string)}
  <ContextMenu.Item onSelect={() => revealInExplorerView(folderPath, path)}>
    <Icon icon={ListTreeIcon} size={14} />
    <span>Reveal in Explorer</span>
  </ContextMenu.Item>
  {#if remoteOk}
    <ContextMenu.Item onSelect={() => revealInFileManager(folderPath, path)}>
      <Icon icon={FolderOpenIcon} size={14} />
      <span>Reveal in File Manager</span>
    </ContextMenu.Item>
  {/if}
{/snippet}

{#snippet copyEntries(path: string, files: string[], showPatch: boolean)}
  <ContextMenu.Item onSelect={() => copyPath(folderPath, path)}>
    <Icon icon={ClipboardIcon} size={14} />
    <span>Copy Path</span>
  </ContextMenu.Item>
  <ContextMenu.Item onSelect={() => copyRelativePath(path)}>
    <Icon icon={ClipboardIcon} size={14} />
    <span>Copy Relative Path</span>
  </ContextMenu.Item>
  {#if showPatch}
    <ContextMenu.Item onSelect={() => copyPatch(folderPath, files, staged, path)}>
      <Icon icon={ClipboardIcon} size={14} />
      <span>Copy Patch</span>
    </ContextMenu.Item>
  {/if}
{/snippet}

{#snippet conflictSideEntries(files: string[], all = false)}
  <ContextMenu.Item disabled={resolving} onSelect={() => void resolveConflicts(files, 'ours')}>
    <Icon icon={ArrowUp02Icon} size={14} />
    <span>Take Current Version{all ? ' (All)' : ''}</span>
  </ContextMenu.Item>
  <ContextMenu.Item disabled={resolving} onSelect={() => void resolveConflicts(files, 'theirs')}>
    <Icon icon={ArrowDown02Icon} size={14} />
    <span>Take Incoming Version{all ? ' (All)' : ''}</span>
  </ContextMenu.Item>
{/snippet}

<ContextMenu.Root>
  <ContextMenu.Trigger class="flex flex-1 flex-col" oncontextmenucapture={onResetTarget}>
    {@render children()}
  </ContextMenu.Trigger>

  <ContextMenu.Content>
    {#if target?.kind === 'conflict'}
      {@const conflict = target.conflict}
      {@const path = conflict.path}
      {#if conflict.kind !== 'both_deleted'}
        <ContextMenu.Item onSelect={() => openActualFile(folderPath, path)}>
          <Icon icon={SquareArrowOutUpRightIcon} size={14} />
          <span>Open File</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Item disabled={resolving} onSelect={() => void resolveConflicts([path])}>
        <Icon icon={CheckIcon} size={14} />
        <span>Mark Resolved</span>
      </ContextMenu.Item>
      <ContextMenu.Separator />
      {@render conflictSideEntries([path])}
      {#if extantConflictPath === path}
        <ContextMenu.Separator />
        {@render revealEntries(path)}
      {/if}
      <ContextMenu.Separator />
      {@render copyEntries(path, [path], false)}
    {:else if target?.kind === 'folder' && target.section === 'merge'}
      {@const folder = target}
      <ContextMenu.Item disabled={resolving} onSelect={() => void resolveConflicts(folder.files)}>
        <Icon icon={CheckIcon} size={14} />
        <span>Mark Resolved</span>
      </ContextMenu.Item>
      <ContextMenu.Separator />
      {@render conflictSideEntries(folder.files)}
      <ContextMenu.Separator />
      {@render revealEntries(folder.path)}
      <ContextMenu.Separator />
      <ContextMenu.Item onSelect={() => copyPath(folderPath, folder.path)}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Path</span>
      </ContextMenu.Item>
    {:else if target?.kind === 'section' && target.section === 'merge'}
      {@const section = target}
      <ContextMenu.Item disabled={resolving} onSelect={() => void resolveConflicts(section.files)}>
        <Icon icon={CheckIcon} size={14} />
        <span>Mark All Resolved</span>
      </ContextMenu.Item>
      <ContextMenu.Separator />
      {@render conflictSideEntries(section.files, true)}
      <ContextMenu.Separator />
      <ContextMenu.Item onSelect={() => copyRelativePath(section.files.join('\n'))}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Relative Path</span>
      </ContextMenu.Item>
    {:else if target?.kind === 'file'}
      {@const change = target.change}
      {@const path = change.path}
      <ContextMenu.Item onSelect={() => openWorkingTreeDiff(folderPath, staged, path, path, { temporary: false })}>
        <Icon icon={FileDiffIcon} size={14} />
        <span>Open Changes</span>
      </ContextMenu.Item>
      {#if change.status !== 'D'}
        <ContextMenu.Item onSelect={() => openActualFile(folderPath, path)}>
          <Icon icon={SquareArrowOutUpRightIcon} size={14} />
          <span>Open File</span>
        </ContextMenu.Item>
      {/if}
      {#if pathGitState(summary, path, 'file')?.inHead}
        <ContextMenu.Item onSelect={() => openHeadFile(folderPath, path)}>
          <Icon icon={EyeIcon} size={14} />
          <span>Open File (HEAD)</span>
        </ContextMenu.Item>
      {/if}

      <ContextMenu.Separator />
      {#if staged}
        <ContextMenu.Item onSelect={() => runPathsAction(folderPath, 'unstage', [path], path)}>
          <Icon icon={CircleMinusIcon} size={14} />
          <span>Unstage Changes</span>
        </ContextMenu.Item>
      {:else}
        <ContextMenu.Item onSelect={() => runPathsAction(folderPath, 'stage', [path], path)}>
          <Icon icon={CirclePlusIcon} size={14} />
          <span>Stage Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item destructive onSelect={() => confirmDiscardPaths(folderPath, [path], path)}>
          <Icon icon={Delete02Icon} size={14} />
          <span>Discard Changes</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Item onSelect={() => promptStash(folderPath, { kind: 'paths', paths: [path] })}>
        <Icon icon={PackageIcon} size={14} />
        <span>Stash Changes…</span>
      </ContextMenu.Item>
      {#if !staged}
        <ContextMenu.Item onSelect={() => ignoreFiles(folderPath, [path])}>
          <Icon icon={EyeOffIcon} size={14} />
          <span>Add to .gitignore</span>
        </ContextMenu.Item>
      {/if}

      <ContextMenu.Separator />
      {@render revealEntries(path)}
      <ContextMenu.Separator />
      {@render copyEntries(path, [path], staged || change.status !== '?')}
    {:else if target?.kind === 'folder'}
      {@const folder = target}
      {#if staged}
        <ContextMenu.Item onSelect={() => runPathsAction(folderPath, 'unstage', folder.files, describeTarget(folder))}>
          <Icon icon={CircleMinusIcon} size={14} />
          <span>Unstage Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          onSelect={() => openWorkingTreeDiff(folderPath, true, folder.path, null, { temporary: false })}
        >
          <Icon icon={FileDiffIcon} size={14} />
          <span>Open Changes</span>
        </ContextMenu.Item>
      {:else}
        <ContextMenu.Item onSelect={() => runPathsAction(folderPath, 'stage', folder.files, describeTarget(folder))}>
          <Icon icon={CirclePlusIcon} size={14} />
          <span>Stage Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          destructive
          onSelect={() => confirmDiscardPaths(folderPath, folder.files, describeTarget(folder))}
        >
          <Icon icon={Delete02Icon} size={14} />
          <span>Discard Changes</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Item onSelect={() => promptStash(folderPath, { kind: 'paths', paths: [folder.path] })}>
        <Icon icon={PackageIcon} size={14} />
        <span>Stash Changes…</span>
      </ContextMenu.Item>
      {#if !staged}
        <ContextMenu.Item onSelect={() => ignoreFiles(folderPath, [folder.path])}>
          <Icon icon={EyeOffIcon} size={14} />
          <span>Add to .gitignore</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          onSelect={() => openWorkingTreeDiff(folderPath, false, folder.path, null, { temporary: false })}
        >
          <Icon icon={FileDiffIcon} size={14} />
          <span>Open Changes</span>
        </ContextMenu.Item>
      {/if}

      <ContextMenu.Separator />
      {@render revealEntries(folder.path)}
      <ContextMenu.Separator />
      {@render copyEntries(folder.path, staged ? folder.files : trackedFiles, true)}
    {:else if target?.kind === 'section'}
      {@const section = target}
      {@const empty = section.files.length === 0}
      {#if staged}
        <ContextMenu.Item disabled={empty} onSelect={() => unstageAllFolder(folderPath)}>
          <Icon icon={CircleMinusIcon} size={14} />
          <span>Unstage All Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item disabled={empty} onSelect={() => promptStash(folderPath, { kind: 'staged' })}>
          <Icon icon={PackageIcon} size={14} />
          <span>Stash Staged…</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          disabled={empty}
          onSelect={() => openWorkingTreeDiff(folderPath, true, null, null, { temporary: false })}
        >
          <Icon icon={FileDiffIcon} size={14} />
          <span>Open Staged Changes</span>
        </ContextMenu.Item>
      {:else}
        <ContextMenu.Item disabled={empty} onSelect={() => stageAllFolder(folderPath)}>
          <Icon icon={CirclePlusIcon} size={14} />
          <span>Stage All Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item disabled={empty} destructive onSelect={() => confirmDiscardAll(folderPath)}>
          <Icon icon={Delete02Icon} size={14} />
          <span>Discard All Changes</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          disabled={empty}
          onSelect={() => promptStash(folderPath, { kind: 'all', includeUntracked: true })}
        >
          <Icon icon={PackageIcon} size={14} />
          <span>Stash (Include Untracked)…</span>
        </ContextMenu.Item>
        <ContextMenu.Item
          disabled={empty}
          onSelect={() => openWorkingTreeDiff(folderPath, false, null, null, { temporary: false })}
        >
          <Icon icon={FileDiffIcon} size={14} />
          <span>Open Changes</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Separator />
      <ContextMenu.Item
        disabled={empty}
        onSelect={() => copyPatch(folderPath, staged ? section.files : trackedFiles, staged, 'this section')}
      >
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Patch</span>
      </ContextMenu.Item>
    {:else}
      {@const noUnstaged = summary.unstaged_count + summary.untracked_count === 0}
      <ContextMenu.Item onSelect={() => syncFolder(folderPath)}>
        <Icon icon={RefreshCwIcon} size={14} />
        <span>Sync</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={() => pullFolder(folderPath)}>
        <Icon icon={ArrowDown02Icon} size={14} />
        <span>Pull</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={() => pushFolder(folderPath)}>
        <Icon icon={ArrowUp02Icon} size={14} />
        <span>Push</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={() => fetchFolder(folderPath)}>
        <Icon icon={RefreshIcon} size={14} />
        <span>Fetch</span>
      </ContextMenu.Item>
      <ContextMenu.Sub>
        <ContextMenu.SubTrigger>
          <Icon icon={ArrowDataTransferVerticalIcon} size={14} />
          <span>Pull, Push</span>
        </ContextMenu.SubTrigger>
        <ContextMenu.SubContent>
          <ContextMenu.Item onSelect={() => syncFolder(folderPath, true)}>
            <Icon icon={RefreshCwIcon} size={14} />
            <span>Sync (Rebase)</span>
          </ContextMenu.Item>
          <ContextMenu.Item onSelect={() => pullFolder(folderPath, true)}>
            <Icon icon={ArrowDown02Icon} size={14} />
            <span>Pull (Rebase)</span>
          </ContextMenu.Item>
          <ContextMenu.Item destructive onSelect={() => forcePushWithLease(folderPath)}>
            <Icon icon={ArrowUp02Icon} size={14} />
            <span>Force Push (with lease)</span>
          </ContextMenu.Item>
          <ContextMenu.Separator />
          <ContextMenu.Item
            disabled={!summary.branch || summary.detached || summary.unborn || !!summary.upstream}
            onSelect={() => publishBranch(folderPath)}
          >
            <Icon icon={CloudUploadIcon} size={14} />
            <span>Publish Branch…</span>
          </ContextMenu.Item>
        </ContextMenu.SubContent>
      </ContextMenu.Sub>

      <ContextMenu.Separator />
      <ContextMenu.Item disabled={noUnstaged} onSelect={() => stageAllFolder(folderPath)}>
        <Icon icon={CirclePlusIcon} size={14} />
        <span>Stage All Changes</span>
      </ContextMenu.Item>
      <ContextMenu.Item disabled={summary.staged_count === 0} onSelect={() => unstageAllFolder(folderPath)}>
        <Icon icon={CircleMinusIcon} size={14} />
        <span>Unstage All Changes</span>
      </ContextMenu.Item>
      <ContextMenu.Item disabled={noUnstaged} destructive onSelect={() => confirmDiscardAll(folderPath)}>
        <Icon icon={Delete02Icon} size={14} />
        <span>Discard All Changes</span>
      </ContextMenu.Item>

      <ContextMenu.Separator />
      <ContextMenu.Item
        disabled={summary.staged_count + summary.unstaged_count === 0}
        onSelect={() => promptStash(folderPath, { kind: 'all', includeUntracked: false })}
      >
        <Icon icon={PackageIcon} size={14} />
        <span>Stash…</span>
      </ContextMenu.Item>
      <ContextMenu.Item
        disabled={summary.changes.length === 0}
        onSelect={() => promptStash(folderPath, { kind: 'all', includeUntracked: true })}
      >
        <Icon icon={PackageIcon} size={14} />
        <span>Stash (Include Untracked)…</span>
      </ContextMenu.Item>
      <ContextMenu.Item
        disabled={summary.staged_count === 0}
        onSelect={() => promptStash(folderPath, { kind: 'staged' })}
      >
        <Icon icon={PackageIcon} size={14} />
        <span>Stash Staged…</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={() => popLatestStash(folderPath)}>
        <Icon icon={PackageOpenIcon} size={14} />
        <span>Pop Latest Stash</span>
      </ContextMenu.Item>

      <ContextMenu.Separator />
      <ContextMenu.Item
        disabled={noUnstaged}
        onSelect={() => openWorkingTreeDiff(folderPath, false, null, null, { temporary: false })}
      >
        <Icon icon={FileDiffIcon} size={14} />
        <span>Open Changes</span>
      </ContextMenu.Item>
      {#if !summary.unborn && summary.operation === null}
        <ContextMenu.Item destructive onSelect={() => undoLastCommit(folderPath)}>
          <Icon icon={Undo02Icon} size={14} />
          <span>Undo Last Commit</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Separator />
      <ContextMenu.Item onSelect={() => copyFullPatch(folderPath)}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Patch</span>
      </ContextMenu.Item>
    {/if}
  </ContextMenu.Content>
</ContextMenu.Root>
