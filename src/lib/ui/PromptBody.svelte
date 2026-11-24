<script lang="ts">
  import { untrack } from 'svelte';
  import { dialogs, type PromptOptions } from './overlays.svelte';

  interface Props {
    prompt: PromptOptions;
    /** Форма, которую отправляет кнопка диалога. */
    formId: string;
  }

  let { prompt, formId }: Props = $props();

  let value = $state(untrack(() => prompt.value ?? ''));
  let error = $state<string | null>(null);

  function onsubmit(e: SubmitEvent) {
    e.preventDefault();
    error = prompt.validate?.(value) ?? null;
    if (!error) dialogs.submit(value);
  }
</script>

{#if prompt.message}<p class="message">{prompt.message}</p>{/if}
<form id={formId} {onsubmit} novalidate>
  <label class="lbl" for="{formId}-input">{prompt.label}</label>
  <div class="inp" class:invalid={error}>
    <input
      id="{formId}-input"
      bind:value
      oninput={() => (error = null)}
      onfocus={(e) => e.currentTarget.select()}
      data-autofocus
      autocomplete="off"
      spellcheck="false"
      aria-invalid={error ? true : undefined}
      aria-describedby={error ? `${formId}-error` : undefined}
    />
  </div>
  {#if error}<div class="field-error" id="{formId}-error" role="alert">{error}</div>{/if}
</form>

<style>
  .message {
    margin-bottom: 14px;
  }

  form {
    margin-top: 4px;
  }
</style>
