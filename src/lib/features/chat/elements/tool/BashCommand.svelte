<script lang="ts">
  import { IconButton } from '$lib/components/ui/button'
  import { CopyIcon } from '$lib/icons/lucideExports'
  import { copyToClipboard } from '$lib/utils/clipboard'
  import { codeTokens } from '$lib/utils/syntaxHighlight'

  let { command, inline = false }: { command: string; inline?: boolean } = $props()
  const renderedCommand = $derived(inline ? command.trim().split(/\r?\n/, 1)[0] : command)

  type CommandToken = { text: string; color?: string }
  let highlighted = $state.raw<{ command: string; tokens: CommandToken[] } | null>(null)

  $effect(() => {
    const source = renderedCommand
    let cancelled = false
    async function highlight(): Promise<void> {
      const lines = await codeTokens(source, 'shellscript')
      if (cancelled || !lines) return
      const tokens: CommandToken[] = []
      let offset = 0
      // Slice the original source, including gaps between tokens, so line endings
      // and whitespace never depend on Shiki's line-oriented representation.
      for (const line of lines) {
        for (const token of line) {
          if (token.offset > offset) tokens.push({ text: source.slice(offset, token.offset) })
          const end = token.offset + token.content.length
          tokens.push({ text: source.slice(token.offset, end), color: token.color })
          offset = end
        }
      }
      if (offset < source.length) tokens.push({ text: source.slice(offset) })
      if (!cancelled) highlighted = { command: source, tokens }
    }
    void highlight().catch((cause) => console.warn('Bash syntax highlighting failed', cause))
    return () => {
      cancelled = true
    }
  })
</script>

{#snippet source()}
  {#if highlighted?.command === renderedCommand}{#each highlighted.tokens as token}<span style:color={token.color}
        >{token.text}</span
      >{/each}{:else}{renderedCommand}{/if}
{/snippet}

{#if inline}
  <code data-slot="bash-command-inline" class="block min-w-0 truncate font-mono text-xs text-fg"
    >{@render source()}{#if command.trim().includes('\n')}<span class="text-subtle"> …</span>{/if}</code
  >
{:else}
  <div data-slot="bash-command" class="flex min-w-0 items-start gap-2 font-mono text-xs leading-normal">
    <span class="shrink-0 text-subtle select-none" aria-hidden="true">$</span>
    <pre
      data-slot="bash-command-source"
      class="m-0 min-w-0 flex-1 cursor-text font-mono text-xs leading-normal wrap-anywhere whitespace-pre-wrap text-fg"><code
        >{@render source()}</code
      ></pre>
    <IconButton tooltip="Copy Command" class="shrink-0" onclick={() => void copyToClipboard(command)}
      ><CopyIcon size={12} /></IconButton
    >
  </div>
{/if}
