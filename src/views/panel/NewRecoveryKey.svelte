<script lang="ts">
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { dialogs } from '../../lib/ui/overlays.svelte';
  import RecoveryKeyView from './RecoveryKeyView.svelte';

  interface Props {
    key: string;
  }

  let { key }: Props = $props();

  // Старый ключ уже не действует: уйти, не сохранив новый, — только явно.
  function confirmLeave(): Promise<boolean> {
    return dialogs.confirm({
      title: 'Закрыть без сохранения ключа?',
      message:
        'Старый ключ уже не действует. Без нового ключа восстановить доступ, если забудете мастер-пароль, будет нельзя.',
      confirm: 'Закрыть',
      cancel: 'Вернуться',
      danger: true,
    });
  }

  $effect(() => {
    session.guard = confirmLeave;
    return () => {
      if (session.guard === confirmLeave) session.guard = null;
    };
  });
</script>

<main class="new-key">
  <header data-tauri-drag-region>
    <button
      type="button"
      class="ib"
      aria-label="Назад (Esc)"
      title="Назад (Esc)"
      onclick={() => session.leave()}
    >
      <Icon name="back" />
    </button>
    <h1 data-tauri-drag-region>Новый ключ восстановления</h1>
  </header>

  <div class="content">
    <p>
      Старый ключ больше не действует. Сохраните новый: он откроет хранилище, если вы забудете
      мастер-пароль.
    </p>
    <RecoveryKeyView {key} doneLabel="Готово" ondone={() => session.back()} />
  </div>
</main>

<style>
  .new-key {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 4px;
  }

  h1 {
    flex: 1;
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    padding: 4px 16px 16px;
    overflow-y: auto;
  }
</style>
