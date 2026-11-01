<script lang="ts">
  import type { ChatContent } from '$lib/types/backend'

  let { part }: { part: Extract<ChatContent, { type: 'image' }> } = $props()
</script>

<img
  class="max-h-96 max-w-full self-start object-contain"
  src={`data:${part.mimeType};base64,${part.data}`}
  alt="Tool result"
/>
