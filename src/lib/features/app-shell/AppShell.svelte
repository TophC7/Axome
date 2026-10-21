<script lang="ts">
  import { onMount } from 'svelte'
  import { platform } from '$lib/platform'
  import * as terminals from '$lib/features/terminal/registry'
  import { backend } from '$lib/api/backend'
  import { refreshIssuesForFolder } from '$lib/features/issues/state.svelte'
  import { refreshNixForFolder } from '$lib/features/settings/state/nix.svelte'
  import { refreshHarnessListings } from '$lib/features/sessions/harnesses/state.svelte'
  import { refreshShellListings } from '$lib/features/shells/state.svelte'
  import CommandCenter from '$lib/features/command-palette/CommandCenter.svelte'
  import ConfirmHost from '$lib/features/confirm/ConfirmHost.svelte'
  import GitDialogHost from '$lib/features/git/dialogs/GitDialogHost.svelte'
  import { confirmAsync } from '$lib/features/confirm/service.svelte'
  import NotificationsSurface from '$lib/features/notifications/NotificationsSurface.svelte'
  import Browser from '$lib/features/browser/Browser.svelte'
  import { getErrorMessage } from '$lib/utils/client-error'
  import SettingsDialog from '$lib/features/settings/dialog/SettingsDialog.svelte'
  import { loadSettings } from '$lib/features/settings/state/settings.svelte'
  import {
    getCodeFontFamily,
    getTerminalFontFamily,
    getTerminalFontSize,
    getUiFontFamily
  } from '$lib/features/settings/state/typography.svelte'
  import { applyTerminalFont } from '$lib/features/terminal/xterm'
  import TitleBar from '$lib/features/app-shell/titlebar/TitleBar.svelte'
  import * as Tooltip from '$lib/components/ui/tooltip'
  import { initWindowControls } from '$lib/features/app-shell/window-controls/state.svelte'
  import { isSettingsOpen, setSettingsOpen } from '$lib/features/settings/dialog/state.svelte'
  import { isAnyModalOpen } from '$lib/utils/modalRegistry.svelte'
  import { setupGlobalShortcuts } from '$lib/features/command-palette/shortcuts/keybindings.svelte'
  import { confirmDiscardDirtyText } from '$lib/features/workbench/surfaces/text/service.svelte'
  import {
    flushWorkbench,
    getActiveFolderPath,
    getActiveTerminalTabId,
    requestFocusTab
  } from '$lib/features/workbench/state.svelte'
  import { notify } from '$lib/features/notifications/state.svelte'
  import {
    ensureSettingsDiagnosticsListener,
    refreshSettingsDiagnostics
  } from '$lib/features/settings/state/diagnostics.svelte'
  import { initTransferService } from '$lib/features/workbench/transferService.svelte'
  import type { Snippet } from 'svelte'
  import { initDeepLinks } from '$lib/features/remotes/deepLink.svelte'

  let { children }: { children: Snippet } = $props()

  // Keep xterm's textarea focus aligned with the active terminal.
  //
  // Two classes of problem this solves:
  //   1. Tab switches. When the user moves between terminals, a
  //      different TerminalSurface becomes visible but the previously
  //      clicked terminal keeps real DOM focus. Visible-but-unfocused
  //      xterm means Shift+Tab falls through to the browser's focus-
  //      navigation instead of reaching the PTY — feels like "terminal
  //      keys stopped working on this tab."
  //   2. Transient modals (palette, settings). bits-ui restores focus
  //      to the activeElement captured at open time. If the command
  //      that ran mutated the DOM (new tab, close tab, re-attach),
  //      that reference is stale and focus falls to <body>.
  //
  // One effect tracks both signals. We refocus the current active
  // terminal's xterm on any change, unless a transient modal is
  // currently open — in which case the modal owns keyboard focus and
  // the effect will fire again when it closes. rAF waits one frame so
  // bits-ui's own restoration attempt completes first and we override
  // it last.
  const activeTerminalTabId = $derived(getActiveTerminalTabId())
  const anyModalOpen = $derived(isAnyModalOpen())

  $effect(() => {
    const id = activeTerminalTabId
    const open = anyModalOpen
    if (open || !id || !document.hasFocus()) return
    requestAnimationFrame(() => {
      if (document.hasFocus()) terminals.focus(id)
    })
  })

  // Typography settings override the bundled @theme font tokens, so every
  // `font-sans` / `font-mono` utility follows them without a reload.
  $effect(() => {
    const root = document.documentElement.style
    root.setProperty('--font-sans', getUiFontFamily())
    root.setProperty('--font-mono', getCodeFontFamily())
  })

  $effect(() => void applyTerminalFont(getTerminalFontFamily(), getTerminalFontSize()))

  // Settings diagnostics are app-global: the rail's Settings menu shows them
  // with or without a folder open. Re-read whenever the active folder changes.
  $effect(() => {
    ensureSettingsDiagnosticsListener()
    const requestedFolderPath = getActiveFolderPath()
    let disposed = false
    void refreshSettingsDiagnostics(requestedFolderPath ?? undefined).catch((error) => {
      if (!disposed && requestedFolderPath === getActiveFolderPath()) {
        notify.error('Failed to load settings diagnostics', getErrorMessage(error))
      }
    })
    return () => {
      disposed = true
    }
  })

  onMount(() => {
    const native = platform.native
    const cleanupDeepLinks = native ? initDeepLinks() : undefined
    const cleanupWindowControls = initWindowControls()
    let cleanupTransfer: (() => void) | undefined
    let disposed = false
    const listeners = [
      backend.settings.onChanged((event) => {
        if (event.layer === 'global') void loadSettings()
        refreshHarnessListings(event)
        refreshShellListings(event)
      }),
      backend.issues.onChanged(({ folderPath }) => void refreshIssuesForFolder(folderPath)),
      backend.nix.onChanged(({ folderPath }) => refreshNixForFolder(folderPath)),
      ...(native ? [native.window.onFocusTab((payload) => requestFocusTab(payload.tabId, payload.reveal))] : [])
    ]
    if (native) {
      void initTransferService().then((cleanup) => {
        if (disposed) cleanup()
        else cleanupTransfer = cleanup
      })
    }
    void loadSettings()

    const unlisten = native?.window.onCloseRequested(async (event) => {
      // Guard before any teardown — once we've started flushing the
      // user has effectively committed to closing.
      if (!(await confirmDiscardDirtyText('Quit'))) {
        event.preventDefault()
        return
      }

      // Persist pending workbench mutations before tearing down — the
      // backend knows nothing about the frontend's debounce queue. A
      // failed write would silently lose the layout, so let the user
      // choose between quitting anyway and keeping the app open.
      try {
        await flushWorkbench()
      } catch (error) {
        const proceed = await confirmAsync({
          title: 'Could not save workbench layout',
          message: `${getErrorMessage(error)}\n\nQuit anyway? Open tabs will not be restored.`,
          confirmLabel: 'Quit',
          cancelLabel: 'Keep working'
        })
        if (!proceed) {
          event.preventDefault()
          return
        }
      }
      terminals.releaseAll()
    })

    const cleanupShortcuts = setupGlobalShortcuts()

    return () => {
      disposed = true
      cleanupDeepLinks?.()
      cleanupTransfer?.()
      cleanupWindowControls()
      cleanupShortcuts()
      for (const listener of listeners) listener.then((cleanup) => cleanup()).catch(() => {})
      unlisten?.then((cleanup) => cleanup()).catch(() => {})
    }
  })
</script>

<Tooltip.Provider delayDuration={300}>
  <div class="flex h-screen flex-col overflow-hidden">
    <TitleBar />

    <main class="flex min-h-0 flex-1 flex-col overflow-hidden">
      {@render children()}
    </main>
  </div>

  <!-- Dialogs and overlays live under the same Tooltip.Provider as the
       main app so IconButtons inside them (e.g. Settings close) find a
       provider without needing a per-surface one. Nested providers
       confuse bits-ui's cross-tooltip coordination. -->
  <CommandCenter />
  <SettingsDialog open={isSettingsOpen()} onClose={() => setSettingsOpen(false)} />
  <NotificationsSurface />
  <Browser />
  <GitDialogHost />
  <ConfirmHost />
</Tooltip.Provider>
