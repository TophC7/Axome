<script lang="ts">
  import { IconButton } from '$lib/components/ui/button'
  import { openBrowser } from '$lib/features/browser/state.svelte'
  import { getActiveFolderPath, openNewTab } from '$lib/features/workbench/state.svelte'
  import { PlusIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'

  function handleNewTab(): void {
    const folderPath = getActiveFolderPath()
    if (folderPath) openNewTab(folderPath)
    else openBrowser()
  }
</script>

<IconButton size="md" tooltip="New Tab" class="sticky right-0 ml-0.5 shrink-0 bg-surface" onclick={handleNewTab}>
  <Icon icon={PlusIcon} size={14} />
</IconButton>
