<script lang="ts">
  import {
    RAIL_WIDTH_CLASS,
    getSidebarView,
    setSidebarView,
    isSidebarCollapsed,
    setSidebarCollapsed,
    type SidebarView
  } from '$lib/features/app-shell/sidebar/state.svelte'
  import type { ToneStatus } from '$lib/components/ui/status-dot'
  import RailButton from './RailButton.svelte'
  import AppMenu from './applets/AppMenu.svelte'
  import NixApplet from './applets/NixApplet.svelte'
  import NotificationsApplet from './applets/NotificationsApplet.svelte'
  import RemotesApplet from './applets/RemotesApplet.svelte'
  import ServerApplet from './applets/ServerApplet.svelte'
  import { GitBranchIcon, FolderOpenIcon, ClipboardListIcon, MessageSquare } from '$lib/icons/lucideExports'
  import { getGitSummary } from '$lib/features/git/state.svelte'
  import { describeOperation } from '$lib/features/git/operation'
  import { countLiveAgents } from '$lib/features/sessions/service.svelte'
  import { platform } from '$lib/platform'

  let { folderPath }: { folderPath: string | null } = $props()

  let activeView = $derived(getSidebarView())
  let collapsed = $derived(isSidebarCollapsed())

  let gitSummary = $derived(folderPath ? getGitSummary(folderPath) : null)
  // Unique changed-path count for the git badge. `changes` lists a file
  // twice when it has both staged and unstaged edits, so count by path.
  let gitChangeCount = $derived(gitSummary ? new Set(gitSummary.changes.map((c) => c.path)).size : 0)
  // Over 99 we swap to ":D" — both as a playful signal that you have bigger
  // problems than an exact count, and to keep the badge width bounded.
  let gitBadgeLabel = $derived(gitChangeCount > 99 ? ':D' : String(gitChangeCount))
  let sharedAgents = $derived(folderPath ? countLiveAgents(folderPath) : 0)

  let viewStatus = $derived<Partial<Record<SidebarView, ToneStatus>>>({
    sessions:
      sharedAgents > 1 ? { tone: 'waiting', text: `${sharedAgents} agents share this working tree` } : undefined,
    git: gitSummary?.operation ? { tone: 'waiting', text: describeOperation(gitSummary.operation).headline } : undefined
  })

  function handleClick(view: SidebarView) {
    if (activeView === view) {
      // Toggle collapse when clicking the already-active view.
      setSidebarCollapsed(!collapsed)
    } else {
      setSidebarView(view)
      if (collapsed) setSidebarCollapsed(false)
    }
  }

  const views: { id: SidebarView; icon: typeof GitBranchIcon; label: string }[] = [
    { id: 'sessions', icon: MessageSquare, label: 'Sessions' },
    { id: 'files', icon: FolderOpenIcon, label: 'Files' },
    { id: 'git', icon: GitBranchIcon, label: 'Git' },
    { id: 'issues', icon: ClipboardListIcon, label: 'Issues' }
  ]
</script>

<div class="flex {RAIL_WIDTH_CLASS} shrink-0 flex-col items-center gap-0.5 border-r border-edge bg-ground py-2">
  {#if folderPath}
    {#each views as view}
      {@const isActive = activeView === view.id && !collapsed}
      <div class="relative">
        <RailButton
          label={view.label}
          status={viewStatus[view.id] ?? null}
          active={isActive}
          onclick={() => handleClick(view.id)}
        >
          <view.icon size={16} />
        </RailButton>
        <!-- After the button: RailButton is positioned (for its status dot), so an earlier sibling would paint under its active background. -->
        {#if isActive}
          <span class="absolute top-1 bottom-1 left-0 w-0.5 rounded-r bg-accent" aria-hidden="true"></span>
        {/if}
        {#if view.id === 'git' && gitChangeCount > 0}
          <span
            class="pointer-events-none absolute -top-0.5 -right-0.5 inline-flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-accent px-1 font-mono text-2xs leading-none font-semibold text-ground ring-1 ring-ground"
            aria-label="{gitChangeCount} changed files"
          >
            {gitBadgeLabel}
          </span>
        {/if}
      </div>
    {/each}
  {/if}

  <!-- Rail-foot applets (see RailApplet's contract); each renders nothing when irrelevant. -->
  <div class="mt-auto flex flex-col items-center gap-0.5">
    {#if folderPath}<NixApplet {folderPath} />{/if}
    {#if platform.native}<RemotesApplet {folderPath} />{:else}<ServerApplet />{/if}
    <NotificationsApplet />
    <AppMenu />
  </div>
</div>
