<script lang="ts">
  import * as ContextMenu from '$lib/components/ui/context-menu'
  import { ignoreFiles } from '$lib/features/git/actions.svelte'
  import type { PathGitState } from '$lib/features/git/gitStatus'
  import {
    confirmDiscardPaths,
    describeFolder,
    openHeadFile,
    revealInSourceControl,
    runPathsAction
  } from '$lib/features/git/treeActions'
  import {
    type CompareFile,
    canCompareWithSelected,
    compareWithClipboard,
    compareWithSelected,
    selectForCompare
  } from '$lib/features/workbench/surfaces/diff/compare.svelte'
  import { openWorkingTreeDiff } from '$lib/features/workbench/surfaces/diff/service.svelte'
  import {
    FolderOpenIcon,
    FileCodeIcon,
    FileDiffIcon,
    EyeIcon,
    EyeOffIcon,
    CirclePlusIcon,
    CircleMinusIcon,
    GitBranchIcon,
    GitCompareIcon,
    ScissorsIcon,
    CopyIcon,
    ClipboardIcon,
    PencilIcon,
    Delete02Icon,
    PlusIcon,
    SquareArrowOutUpRightIcon,
    ClipboardPasteIcon
  } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import type { Snippet } from 'svelte'

  let {
    folderPath,
    filePath,
    targetType,
    git,
    children,
    canRevealInFileManager,
    canNativeFileClipboard,
    onRevealInFolder,
    onOpenInEditor,
    onCut,
    onCopy,
    onPaste,
    onCopyPath,
    onCopyRelativePath,
    onRename,
    onDelete,
    onNewFile,
    onNewFolder,
    onOpenExternal,
    onCopyFolderPath,
    onResetTarget
  }: {
    folderPath: string
    filePath: string | null
    targetType: 'file' | 'directory' | null
    git: PathGitState | null
    children: Snippet
    canRevealInFileManager: boolean
    canNativeFileClipboard: boolean
    onRevealInFolder: () => void
    onOpenInEditor: () => void
    onCut: () => void
    onCopy: () => void
    onPaste: () => void
    onCopyPath: () => void
    onCopyRelativePath: () => void
    onRename: () => void
    onDelete: () => void
    onNewFile: () => void
    onNewFolder: () => void
    onOpenExternal: () => void
    onCopyFolderPath: () => void
    onResetTarget: () => void
  } = $props()

  const file = $derived<CompareFile | null>(filePath && targetType === 'file' ? { folderPath, path: filePath } : null)
  const changed = $derived(!!git && git.staged.length + git.unstaged.length > 0)
</script>

<ContextMenu.Root>
  <!-- svelte-ignore event_directive_deprecated -->
  <ContextMenu.Trigger class="block min-h-full" oncontextmenucapture={onResetTarget}>
    {@render children()}
  </ContextMenu.Trigger>

  <ContextMenu.Content>
    {#if filePath}
      {@const path = filePath}
      {@const describe = (files: string[]) => (targetType === 'file' ? path : describeFolder(path, files.length))}
      <!-- ── File/folder-targeted menu ── -->
      {#if canRevealInFileManager}
        <ContextMenu.Item onSelect={onRevealInFolder}>
          <Icon icon={FolderOpenIcon} size={14} />
          <span>Reveal in File Manager</span>
        </ContextMenu.Item>
      {/if}

      {#if targetType === 'file'}
        {#if canRevealInFileManager}
          <ContextMenu.Separator />
        {/if}

        <ContextMenu.Item onSelect={onOpenInEditor}>
          <Icon icon={FileCodeIcon} size={14} />
          <span>Open in Editor</span>
        </ContextMenu.Item>

        {#if git && changed}
          {@const state = git}
          <ContextMenu.Item
            onSelect={() =>
              openWorkingTreeDiff(folderPath, state.unstaged.length === 0, path, path, { temporary: false })}
          >
            <Icon icon={FileDiffIcon} size={14} />
            <span>Open Changes</span>
          </ContextMenu.Item>
        {/if}
        {#if git?.inHead}
          <ContextMenu.Item onSelect={() => openHeadFile(folderPath, path)}>
            <Icon icon={EyeIcon} size={14} />
            <span>Open File (HEAD)</span>
          </ContextMenu.Item>
        {/if}
      {/if}

      {#if git}
        {@const state = git}
        {#if canRevealInFileManager || targetType === 'file'}
          <ContextMenu.Separator />
        {/if}
        {#if targetType === 'directory' && changed}
          <ContextMenu.Item
            onSelect={() =>
              openWorkingTreeDiff(folderPath, state.unstaged.length === 0, path, null, { temporary: false })}
          >
            <Icon icon={FileDiffIcon} size={14} />
            <span>Open Changes</span>
          </ContextMenu.Item>
        {/if}
        {#if state.unstaged.length > 0}
          <ContextMenu.Item
            onSelect={() => runPathsAction(folderPath, 'stage', state.unstaged, describe(state.unstaged))}
          >
            <Icon icon={CirclePlusIcon} size={14} />
            <span>Stage Changes</span>
          </ContextMenu.Item>
        {/if}
        {#if state.staged.length > 0}
          <ContextMenu.Item
            onSelect={() => runPathsAction(folderPath, 'unstage', state.staged, describe(state.staged))}
          >
            <Icon icon={CircleMinusIcon} size={14} />
            <span>Unstage Changes</span>
          </ContextMenu.Item>
        {/if}
        {#if state.unstaged.length > 0}
          <ContextMenu.Item
            destructive
            onSelect={() => confirmDiscardPaths(folderPath, state.unstaged, describe(state.unstaged))}
          >
            <Icon icon={Delete02Icon} size={14} />
            <span>Discard Changes</span>
          </ContextMenu.Item>
          <ContextMenu.Item onSelect={() => ignoreFiles(folderPath, [path])}>
            <Icon icon={EyeOffIcon} size={14} />
            <span>Add to .gitignore</span>
          </ContextMenu.Item>
        {/if}
        <ContextMenu.Item onSelect={() => revealInSourceControl(folderPath, path)}>
          <Icon icon={GitBranchIcon} size={14} />
          <span>Reveal in Source Control</span>
        </ContextMenu.Item>
      {/if}

      {#if file}
        {@const compareFile = file}
        <ContextMenu.Separator />
        <ContextMenu.Item onSelect={() => selectForCompare(compareFile)}>
          <Icon icon={GitCompareIcon} size={14} />
          <span>Select for Compare</span>
        </ContextMenu.Item>
        {#if canCompareWithSelected(compareFile)}
          <ContextMenu.Item onSelect={() => compareWithSelected(compareFile)}>
            <Icon icon={GitCompareIcon} size={14} />
            <span>Compare with Selected</span>
          </ContextMenu.Item>
        {/if}
        <ContextMenu.Item onSelect={() => void compareWithClipboard(compareFile)}>
          <Icon icon={ClipboardPasteIcon} size={14} />
          <span>Compare with Clipboard</span>
        </ContextMenu.Item>
      {/if}

      {#if (canRevealInFileManager || targetType === 'file' || git !== null) && canNativeFileClipboard}
        <ContextMenu.Separator />
      {/if}

      {#if canNativeFileClipboard}
        <ContextMenu.Item onSelect={onCut}>
          <Icon icon={ScissorsIcon} size={14} />
          <span>Cut</span>
        </ContextMenu.Item>
        <ContextMenu.Item onSelect={onCopy}>
          <Icon icon={CopyIcon} size={14} />
          <span>Copy</span>
        </ContextMenu.Item>
        {#if targetType === 'directory'}
          <ContextMenu.Item onSelect={onPaste}>
            <Icon icon={ClipboardPasteIcon} size={14} />
            <span>Paste</span>
          </ContextMenu.Item>
        {/if}
      {/if}

      {#if canNativeFileClipboard}<ContextMenu.Separator />{/if}

      <ContextMenu.Item onSelect={onCopyPath}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Path</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={onCopyRelativePath}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Relative Path</span>
      </ContextMenu.Item>

      <ContextMenu.Separator />

      <ContextMenu.Item onSelect={onRename}>
        <Icon icon={PencilIcon} size={14} />
        <span>Rename</span>
      </ContextMenu.Item>
      <ContextMenu.Item destructive onSelect={onDelete}>
        <Icon icon={Delete02Icon} size={14} />
        <span>Delete</span>
      </ContextMenu.Item>
    {:else}
      <!-- ── Empty space menu ── -->
      <ContextMenu.Item onSelect={onNewFile}>
        <Icon icon={PlusIcon} size={14} />
        <span>New File</span>
      </ContextMenu.Item>
      <ContextMenu.Item onSelect={onNewFolder}>
        <Icon icon={FolderOpenIcon} size={14} />
        <span>New Folder</span>
      </ContextMenu.Item>
      <ContextMenu.Separator />

      {#if canNativeFileClipboard}
        <ContextMenu.Item onSelect={onPaste}>
          <Icon icon={ClipboardPasteIcon} size={14} />
          <span>Paste</span>
        </ContextMenu.Item>
      {/if}

      {#if canNativeFileClipboard}<ContextMenu.Separator />{/if}

      {#if canRevealInFileManager}
        <ContextMenu.Item onSelect={onOpenExternal}>
          <Icon icon={SquareArrowOutUpRightIcon} size={14} />
          <span>Reveal in File Manager</span>
        </ContextMenu.Item>
      {/if}
      <ContextMenu.Item onSelect={onCopyFolderPath}>
        <Icon icon={ClipboardIcon} size={14} />
        <span>Copy Path</span>
      </ContextMenu.Item>
    {/if}
  </ContextMenu.Content>
</ContextMenu.Root>
