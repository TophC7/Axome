<script lang="ts">
  import { Button } from '$lib/components/ui/button'
  import { Input } from '$lib/components/ui/input'
  import type { ChatAnswer, ChatQuestion, ChatQuestions } from '$lib/types/backend'

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

  function submit(event: SubmitEvent) {
    event.preventDefault()
    if (complete) onAnswer(questions.callId, answers)
  }
</script>

<form data-slot="ask-card" class="flex flex-col gap-4 rounded-lg border border-edge bg-surface p-3" onsubmit={submit}>
  {#each questions.questions as question, index (question.id)}
    <fieldset class="flex min-w-0 flex-col gap-2">
      <legend class="mb-2 flex flex-col items-start gap-1.5">
        {#if question.header}
          <span class="rounded-sm bg-raised px-1.5 py-0.5 text-xs text-muted">{question.header}</span>
        {/if}
        <span class="text-base text-fg">{question.question}</span>
      </legend>
      <div class="flex flex-col gap-1.5">
        {#each question.options as option, optionIndex (option.label)}
          {@const selected = selections[key(question.id)]?.includes(option.label) ?? false}
          <Button
            type="button"
            variant="outline"
            aria-pressed={selected}
            class={selected
              ? 'h-auto justify-start border-accent bg-accent-bg p-2.5 text-left'
              : 'h-auto justify-start bg-raised p-2.5 text-left'}
            onclick={() => toggle(question, option.label)}
          >
            <span class="flex min-w-0 flex-1 flex-col gap-1">
              <span class="flex flex-wrap items-center gap-2">
                <span class="text-sm text-fg">{option.label}</span>
                {#if question.recommended === optionIndex}
                  <span class="text-xs text-muted">Recommended</span>
                {/if}
              </span>
              {#if option.description}<span class="text-sm font-normal text-muted">{option.description}</span>{/if}
              {#if option.preview}<span class="font-mono text-xs font-normal whitespace-pre-wrap text-muted"
                  >{option.preview}</span
                >{/if}
            </span>
          </Button>
        {/each}
      </div>
      <label for={`${id}-${index}-other`} class="text-sm text-muted">Other</label>
      <Input
        id={`${id}-${index}-other`}
        value={custom[key(question.id)] ?? ''}
        oninput={(event) => {
          custom[key(question.id)] = event.currentTarget.value
        }}
      />
    </fieldset>
  {/each}
  <div class="flex items-center gap-1.5">
    <Button type="submit" variant="accent" size="sm" disabled={!complete}>Submit</Button>
    <Button type="button" variant="ghost" size="sm" onclick={() => onDismiss(questions.callId)}>Dismiss</Button>
  </div>
</form>
