<script lang="ts">
  import Dialog from './Dialog.svelte';
  import { dialogs } from './overlays.svelte';
  import PromptBody from './PromptBody.svelte';

  const formId = $props.id();
</script>

{#if dialogs.current?.kind === 'confirm'}
  {@const current = dialogs.current}
  <Dialog title={current.title} onclose={() => dialogs.close(false)}>
    <p>{current.message}</p>
    {#snippet actions()}
      <button
        type="button"
        class="btn"
        class:danger={current.danger}
        class:accent={!current.danger}
        data-autofocus={current.danger ? undefined : true}
        onclick={() => dialogs.close(true)}
      >
        {current.confirm}
      </button>
      <!-- Для необратимых действий фокус на «Отмене»: случайный Enter ничего не удалит. -->
      <button
        type="button"
        class="btn"
        data-autofocus={current.danger ? true : undefined}
        onclick={() => dialogs.close(false)}
      >
        {current.cancel ?? 'Отмена'}
      </button>
    {/snippet}
  </Dialog>
{:else if dialogs.current?.kind === 'prompt'}
  {@const current = dialogs.current}
  <Dialog title={current.title} onclose={() => dialogs.close(false)}>
    {#key current}
      <PromptBody prompt={current} {formId} />
    {/key}
    {#snippet actions()}
      <button type="submit" class="btn accent" form={formId}>{current.confirm}</button>
      <button type="button" class="btn" onclick={() => dialogs.close(false)}>Отмена</button>
    {/snippet}
  </Dialog>
{/if}
