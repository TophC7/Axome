<!--
  @component
  ThinkingBlock — the model's reasoning as small monospace markdown, distinct
  from tool rows: no header, no gutter. Collapsed it is one line cut off with an
  ellipsis; clicking expands it to the full text and clicking again folds it.
-->

<script lang="ts">
  import * as Message from '$lib/features/chat/elements/message'
  import type { BlockView } from '$lib/features/chat/view'
  import type { ThoughtStep } from './groupSteps'

  let {
    thought,
    view,
    onView,
    folderPath
  }: {
    thought: ThoughtStep
    view: BlockView
    onView: (stepId: string, view: BlockView) => void
    folderPath: string
  } = $props()
  const text = $derived(thought.text)
  const line = $derived(text.replace(/\s+/g, ' ').trim())
  // Only a cut-off or multi-line thought has more to show; a line that fits is just text.
  const multiline = $derived(/\n\s*\S/.test(text.trim()))
  let truncated = $state(false)
  const expandable = $derived(multiline || truncated)

  function measure(element: HTMLElement) {
    void line
    const check = () => (truncated = element.scrollWidth > element.clientWidth)
    check()
    const observer = new ResizeObserver(check)
    observer.observe(element)
    return () => observer.disconnect()
  }

  function fold(event: MouseEvent) {
    // Selecting text or following a rendered link must not fold the block.
    if (window.getSelection()?.toString() || (event.target as Element).closest('a, button')) return
    onView(thought.id, 'collapsed')
  }
</script>

<!-- Collapsed line and expanded markdown share line height and padding so expanding never shifts a line that fit. -->
<div
  data-slot="thinking-block"
  data-collapsed={view === 'collapsed'}
  class="min-w-0 font-mono text-xs leading-normal text-muted italic"
>
  {#if view === 'collapsed'}
    <button
      type="button"
      disabled={!expandable}
      aria-expanded="false"
      aria-label={expandable ? 'Show thinking' : undefined}
      title={expandable ? 'Show thinking' : undefined}
      class="block w-full truncate py-0.5 text-left focus-visible:shadow-focus-ring focus-visible:outline-none enabled:cursor-pointer enabled:hover:text-fg"
      {@attach measure}
      onclick={() => onView(thought.id, 'preview')}>{line}</button
    >
  {:else}
    <button
      type="button"
      aria-expanded="true"
      class="sr-only focus-visible:not-sr-only focus-visible:shadow-focus-ring focus-visible:outline-none"
      onclick={() => onView(thought.id, 'collapsed')}>Hide thinking</button
    >
    <!-- The click target folds for the mouse; the button before it is the keyboard fold. -->
    <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
    <div class="cursor-pointer py-0.5" onclick={fold}>
      <!-- Markdown's unlayered defaults otherwise win over these utilities. -->
      <Message.Response
        {text}
        streaming={thought.streaming}
        {folderPath}
        class="gap-1.5 [&_.markdown-body]:font-mono! [&_.markdown-body]:text-xs! [&_.markdown-body]:leading-normal! [&_.markdown-body]:text-muted! [&_.markdown-body]:italic! [&_.markdown-body>:first-child]:mt-0! [&_.markdown-body>:last-child]:mb-0!"
      />
    </div>
  {/if}
</div>
