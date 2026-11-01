<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import type { ChatAnswer, ChatQuestion, ChatQuestions } from '$lib/types/backend'
  import { KeyHint } from '../key-hint'

  let {
    questions,
    onAnswer,
    onDismiss
  }: {
    questions: ChatQuestions
    onAnswer: (callId: string, answers: ChatAnswer[]) => void
    onDismiss: (callId: string) => void
  } = $props()

  const id = $props.id()
  let selections = $state<Record<string, string[]>>({})
  let custom = $state<Record<string, string>>({})
  const key = (questionId: string) => `${questions.callId}:${questionId}`
  const answers = $derived(
    questions.questions.map((question): ChatAnswer => ({
      id: question.id,
      selected: selections[key(question.id)] ?? [],
      custom: custom[key(question.id)]?.trim() || null
    }))
  )
  const complete = $derived(answers.every((answer) => answer.selected.length > 0 || answer.custom !== null))

  function toggle(question: ChatQuestion, label: string) {
    const questionKey = key(question.id)
    const current = selections[questionKey] ?? []
    selections[questionKey] = current.includes(label)
      ? current.filter((selected) => selected !== label)
      : question.multi
        ? [...current, label]
        : [label]
  }

  function submit(event: Event) {
    event.preventDefault()
    if (complete) onAnswer(questions.callId, answers)
  }

  function keydown(event: KeyboardEvent) {
    if (event.isComposing || event.ctrlKey || event.metaKey || event.altKey) return
    if (event.key === 'Escape') {
      event.preventDefault()
      event.stopPropagation()
      onDismiss(questions.callId)
    } else if (event.key === 'Enter') {
      event.stopPropagation()
      // A focused button keeps native Enter activation: options toggle, Dismiss dismisses, Submit submits.
      if (event.target instanceof HTMLElement && event.target.closest('button')) return
      submit(event)
    } else if (/^[1-9]$/.test(event.key)) {
      if (!(event.target instanceof HTMLElement) || event.target.closest('input, textarea, [contenteditable]')) return
      const fieldset = event.target.closest('fieldset')
      const question = questions.questions[Number(fieldset?.dataset.questionIndex ?? 0)]
      const option = question?.options[Number(event.key) - 1]
      if (!option) return
      event.preventDefault()
      event.stopPropagation()
      toggle(question, option.label)
    }
  }
</script>

<!-- Delegated keys: number keys pick options, Enter submits, Esc dismisses; every control inside stays native. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<form
  data-slot="ask-card"
  class="flex flex-col gap-4 outline-none focus-visible:shadow-focus-ring"
  aria-label="Answer questions"
  tabindex="-1"
  {@attach (element) => {
    // Never steal focus from text the user is typing; they can click or Tab into the card.
    if (!document.activeElement?.closest('input, textarea, select, [contenteditable="true"]')) element.focus()
  }}
  onsubmit={submit}
  onkeydown={keydown}
>
  {#each questions.questions as question, index (question.id)}
    <fieldset data-question-index={index} class="flex min-w-0 flex-col gap-2">
      <legend class="mb-2 flex flex-col items-start gap-1">
        {#if question.header}
          <span class="font-mono text-xs text-subtle">{question.header}</span>
        {/if}
        <span class="text-base text-fg">{question.question}</span>
      </legend>
      <div class="flex flex-col">
        {#each question.options as option, optionIndex (option.label)}
          {@const selected = selections[key(question.id)]?.includes(option.label) ?? false}
          <Button
            type="button"
            variant="ghost"
            aria-pressed={selected}
            aria-keyshortcuts={optionIndex < 9 ? String(optionIndex + 1) : undefined}
            class={`h-auto w-full items-start justify-start gap-2 rounded-none px-2 py-1.5 text-left whitespace-normal transition-none ${
              selected ? 'bg-accent-bg text-bright hover:bg-accent-bg' : 'text-fg'
            }`}
            onclick={() => toggle(question, option.label)}
          >
            <span aria-hidden="true" class="w-4 shrink-0 font-mono text-xs text-subtle">{optionIndex + 1}</span>
            <span class="flex min-w-0 flex-1 flex-col gap-1">
              <span class="text-base font-normal">
                {option.label}
                {#if option.description}<span class="text-muted"> — {option.description}</span>{/if}
                {#if question.recommended === optionIndex}
                  <span class="ml-2 text-xs text-subtle">recommended</span>
                {/if}
              </span>
              {#if option.preview}
                <span class="font-mono text-xs font-normal break-words whitespace-pre-wrap text-muted"
                  >{option.preview}</span
                >
              {/if}
            </span>
          </Button>
        {/each}
      </div>
      <div class="flex items-center gap-2">
        <label for={`${id}-${index}-other`} class="text-sm text-muted">Other</label>
        <Input
          id={`${id}-${index}-other`}
          value={custom[key(question.id)] ?? ''}
          oninput={(event) => {
            custom[key(question.id)] = event.currentTarget.value
          }}
        />
      </div>
    </fieldset>
  {/each}
  <div class="flex items-center gap-4">
    <Button type="submit" variant="text" size="sm" class="px-0 text-fg" disabled={!complete}>
      Submit <KeyHint keys="Enter" />
    </Button>
    <Button type="button" variant="text" size="sm" class="px-0" onclick={() => onDismiss(questions.callId)}>
      Dismiss <KeyHint keys="Esc" />
    </Button>
  </div>
</form>
