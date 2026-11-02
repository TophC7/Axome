<!--
  @component
  NotificationsApplet — rail toggle for the notification surface, which
  anchors beside the rail foot; the dot marks pending notifications.
-->

<script lang="ts">
  import RailButton from '../RailButton.svelte'
  import { BellIcon } from '@hugeicons/core-free-icons'
  import Icon from '$lib/icons/Icon.svelte'
  import {
    getNotifications,
    isNotificationCenterOpen,
    toggleNotificationCenter
  } from '$lib/features/notifications/state.svelte'

  let count = $derived(getNotifications().length)
  let expanded = $derived(isNotificationCenterOpen())
</script>

<RailButton
  label="Notifications"
  status={count ? { tone: 'working', text: `${count} notification${count === 1 ? '' : 's'}` } : null}
  active={expanded}
  aria-controls="notifications-surface"
  aria-expanded={expanded}
  data-notifications-toggle="true"
  onclick={toggleNotificationCenter}
>
  <Icon icon={BellIcon} size={16} />
</RailButton>
