<script lang="ts">
  let { keys }: { keys: string } = $props()
</script>

<kbd data-slot="key-hint" class="font-mono text-2xs font-normal text-muted">{keys}</kbd>
