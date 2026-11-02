<script lang="ts">
  import { getWindowControls } from '$lib/features/app-shell/window-controls/state.svelte'
  import { IconButton } from '$lib/components/ui/button'
  import { FullScreenIcon, CollapseIcon, MinusIcon, XIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import { requireNative } from '$lib/platform'

  let config = $derived(getWindowControls())
  let maximized = $state(false)

  const appWindow = requireNative().window

  $effect(() => {
    const unlisten = appWindow.onResized(async () => {
      maximized = await appWindow.isMaximized()
    })
    void appWindow.isMaximized().then((v) => (maximized = v))
    return () => {
      void unlisten.then((fn) => fn())
    }
  })
</script>

{#if !config.useSystemDecorations}
  <div class="flex shrink-0 items-center self-stretch border-l border-edge px-1">
    {#if config.showMinimize}
      <IconButton size="md" tooltip="Minimize" onclick={() => appWindow.minimize()}>
        <Icon icon={MinusIcon} size={12} />
      </IconButton>
    {/if}

    {#if config.showMaximize}
      <IconButton size="md" tooltip={maximized ? 'Restore' : 'Maximize'} onclick={() => appWindow.toggleMaximize()}>
        <Icon icon={maximized ? CollapseIcon : FullScreenIcon} size={10} />
      </IconButton>
    {/if}

    {#if config.showClose}
      <IconButton size="md" tone="danger" tooltip="Close" onclick={() => appWindow.close()}>
        <Icon icon={XIcon} size={11} />
      </IconButton>
    {/if}
  </div>
{/if}
