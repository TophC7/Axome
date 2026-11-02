<!--
  @component
  NixApplet — rail applet for the folder's Nix environment: pick which Nix
  file drives it, see its state and last evaluation, re-evaluate or disable.
  Renders nothing when the folder has no Nix files.
-->

<script lang="ts">
  import RailApplet, { appletHeadingClass, appletRowClass } from './RailApplet.svelte'
  import * as Alert from '$lib/components/ui/alert'
  import { Button } from '$lib/components/ui/button'
  import nixosUrl from '$lib/assets/nixos.svg?url'
  import MaskIcon from '$lib/icons/MaskIcon.svelte'
  import { Check } from '$lib/icons/lucideExports'
  import { Spinner } from '$lib/components/ui/progress'
  import {
    clearNix,
    evaluateNix,
    getNixDetection,
    getNixStatus,
    isNixEvaluating,
    selectNixFile
  } from '$lib/features/settings/state/nix.svelte'
  import { timeAgo } from '$lib/utils/date'

  let { folderPath }: { folderPath: string } = $props()

  let status = $derived(getNixStatus(folderPath))
  let detection = $derived(getNixDetection(folderPath))
  let selected = $derived(detection?.selected ?? null)
  let busy = $derived(isNixEvaluating(folderPath) || selected?.status === 'evaluating')
  let failed = $derived(selected?.status === 'error' || selected?.status === 'timeout')
</script>

{#if status && detection}
  <RailApplet label="Nix environment" {status}>
    {#snippet icon()}
      <MaskIcon src={nixosUrl} width={16} />
    {/snippet}

    <div class={appletHeadingClass}>Source</div>
    <!-- Not role="radio": arrow-key radio semantics would start an evaluation on every press. -->
    <div role="group" aria-label="Nix source" class="flex flex-col">
      {#each detection.detected_files as file (file)}
        {@const current = selected?.nix_file === file}
        <button
          type="button"
          class={appletRowClass}
          disabled={busy}
          aria-current={current ? 'true' : undefined}
          onclick={() => void selectNixFile(folderPath, file)}
        >
          <span
            class="flex size-3.5 shrink-0 items-center justify-center rounded-full border {current
              ? 'border-accent bg-accent text-ground'
              : 'border-edge-strong'}"
          >
            {#if current && busy}
              <Spinner size={9} />
            {:else if current}
              <Check size={9} strokeWidth={3} />
            {/if}
          </span>
          <span class="truncate font-mono {current ? 'text-bright' : 'text-fg'}">{file}</span>
        </button>
      {/each}
    </div>

    {#if failed && selected?.error_message}
      <Alert.Root variant="danger" class="mt-2 max-h-28 overflow-auto"
        ><pre class="font-mono text-2xs whitespace-pre-wrap">{selected.error_message}</pre></Alert.Root
      >
    {/if}

    {#if selected?.evaluated_at && !busy}
      <div class="mt-2 px-2 text-xs text-subtle">Evaluated {timeAgo(selected.evaluated_at)}</div>
    {/if}

    {#snippet actions()}
      {#if selected}
        <Button variant="ghost" size="xs" disabled={busy} onclick={() => void clearNix(folderPath)}>Disable</Button>
        <Button variant="default" size="xs" disabled={busy} onclick={() => void evaluateNix(folderPath)}>
          {failed ? 'Retry' : selected.status === 'ready' ? 'Re-evaluate' : 'Evaluate'}
        </Button>
      {:else}
        <span class="mr-auto text-xs text-subtle">Pick a source to enable it</span>
      {/if}
    {/snippet}
  </RailApplet>
{/if}
