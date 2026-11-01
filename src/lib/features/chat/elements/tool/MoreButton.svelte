<script lang="ts">
  import { Button } from '$lib/components/ui/button'

  let { label, onExpand }: { label: string; onExpand: () => void } = $props()
</script>

<Button
  type="button"
  variant="text"
  size="xs"
  class="justify-start self-start px-0 font-mono font-normal text-subtle hover:text-fg"
  onclick={onExpand}>… {label}</Button
>
