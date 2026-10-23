<!--
  @component
  WorkbenchView — sidebar rail + folder-scoped sidebar panel + the active
  tab's surface. Sidebar chrome (view, width, collapsed) is global; its
  content follows the active tab's folder.
-->

<script lang="ts">
  import { onMount, untrack } from 'svelte'
  import { activateNixFolder } from '$lib/features/settings/state/nix.svelte'
  import { disposeOsDrop, initOsDrop } from '$lib/features/dnd/os-drop'
  import { platform } from '$lib/platform'
  import { dragObserver } from '$lib/features/dnd/observer.svelte'
  import { DND_MIME } from '$lib/features/dnd/payload'
  import { LocalTransfer } from '$lib/features/dnd/transfer.svelte'
  import { activateGitFolder } from '$lib/features/git/state.svelte'
  import { activateSessionsFolder } from '$lib/features/sessions/state.svelte'
  import SidebarRail from '$lib/features/app-shell/sidebar/SidebarRail.svelte'
  import WorkbenchHome from '$lib/features/home/WorkbenchHome.svelte'
  import GitSidebar from '$lib/features/git/GitSidebar.svelte'
  import FilesSidebar from '$lib/features/files/FilesSidebar.svelte'
  import IssuesSidebar from '$lib/features/issues/IssuesSidebar.svelte'
  import SessionsSidebar from '$lib/features/sessions/SessionsSidebar.svelte'
  import SidebarFooter from '$lib/features/app-shell/sidebar/SidebarFooter.svelte'
  import { ResizeDivider } from '$lib/components/ui/resize-divider'
  import {
    getShownSidebarFolder,
    getSidebarWidth,
    setSidebarWidth,
    getSidebarView
  } from '$lib/features/app-shell/sidebar/state.svelte'
  import SurfaceHost from '$lib/features/workbench/SurfaceHost.svelte'
  import { dropFromOtherWindow } from '$lib/features/workbench/transferService.svelte'
  import { getActiveTab, getTabs } from '$lib/features/workbench/state.svelte'

  let activeTab = $derived(getActiveTab())
  let folderPath = $derived(activeTab?.folderPath ?? null)

  let sidebarFolder = $derived(getShownSidebarFolder())
  let sidebarWidth = $derived(getSidebarWidth())
  let sidebarView = $derived(getSidebarView())
  let sidebarPanelEl = $state<HTMLDivElement | null>(null)
  let tabDropActive = $state(false)
  const foreignTabDropObserver = dragObserver({
    accept: (_payload, types) =>
      platform.native !== null &&
      !LocalTransfer.has('tab') &&
      !LocalTransfer.has('workbench') &&
      (types.includes(DND_MIME.SWORM_TAB) || types.includes(DND_MIME.SWORM_WORKBENCH)),
    onEnter: () => (tabDropActive = true),
    onOver: () => (tabDropActive = true),
    onLeave: () => (tabDropActive = false),
    onDrop: (event) => {
      tabDropActive = false
      dropFromOtherWindow(event, getTabs().length)
    }
  })
  // Panel left edge cached at drag start; it stays fixed while the
  // width changes, so per-move layout reads are unnecessary.
  let panelLeft = 0

  $effect(() => {
    const path = folderPath
    if (!path) return
    // Activation reads folder stores; tracking them would re-activate on every store write.
    return untrack(() => {
      const releaseGit = activateGitFolder(path)
      const releaseSessions = activateSessionsFolder(path)
      return () => {
        releaseGit()
        releaseSessions()
      }
    })
  })

  $effect(() => {
    const path = folderPath
    if (path) untrack(() => void activateNixFolder(path))
  })

  onMount(() => {
    const native = platform.native
    if (!native) return
    void initOsDrop()
    return disposeOsDrop
  })
</script>

<div class="flex min-h-0 flex-1 overflow-hidden">
  <SidebarRail {folderPath} />

  {#if sidebarFolder}
    {#key sidebarFolder}
      <div class="flex shrink-0 flex-col overflow-hidden" style="width: {sidebarWidth}px;" bind:this={sidebarPanelEl}>
        <div class="min-h-0 flex-1 overflow-hidden">
          {#if sidebarView === 'sessions'}
            <SessionsSidebar folderPath={sidebarFolder} />
          {:else if sidebarView === 'git'}
            <GitSidebar folderPath={sidebarFolder} />
          {:else if sidebarView === 'issues'}
            <IssuesSidebar folderPath={sidebarFolder} />
          {:else if sidebarView === 'files'}
            <FilesSidebar folderPath={sidebarFolder} />
          {/if}
        </div>
        <SidebarFooter folderPath={sidebarFolder} />
      </div>
    {/key}

    <!-- Sidebar resize handle: full height beside the left column. -->
    <ResizeDivider
      direction="col"
      onResizeStart={() => (panelLeft = sidebarPanelEl?.getBoundingClientRect().left ?? 0)}
      onResize={(e) => setSidebarWidth(e.clientX - panelLeft)}
      aria-label="Resize sidebar"
    />
  {/if}

  <div
    class="relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden"
    role="region"
    aria-label="Workbench surface"
    {@attach foreignTabDropObserver}
  >
    {#if activeTab}
      <SurfaceHost {activeTab} />
    {:else}
      <WorkbenchHome />
    {/if}
    {#if tabDropActive}
      <div
        class="pointer-events-none absolute inset-0 z-20 border-2 border-accent bg-accent/10"
        role="presentation"
      ></div>
    {/if}
  </div>
</div>
