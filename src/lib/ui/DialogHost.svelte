<script lang="ts">
  import Dialog from './Dialog.svelte';
  import { dialogs } from './overlays.svelte';
</script>

{#if dialogs.current}
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
{/if}
