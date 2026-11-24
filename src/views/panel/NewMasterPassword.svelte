<script lang="ts">
  import { reportError } from '../../lib/actions';
  import { api, ApiError, errorText } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { toasts } from '../../lib/ui/overlays.svelte';
  import PasswordInput from '../../lib/ui/PasswordInput.svelte';
  import MasterPasswordForm from './MasterPasswordForm.svelte';

  interface Props {
    /**
     * `reset` — после входа по ключу восстановления (старый пароль не нужен);
     * `change` — смена из настроек (нужен текущий).
     */
    mode: 'reset' | 'change';
  }

  let { mode }: Props = $props();

  let current = $state('');
  let currentError = $state('');
  let password = $state('');
  let valid = $state(false);
  let busy = $state(false);
  let currentInput: PasswordInput | undefined = $state();
  const id = $props.id();

  const ready = $derived(valid && (mode === 'reset' || current.length > 0));

  async function save() {
    if (!ready || busy) return;
    busy = true;
    currentError = '';
    try {
      if (mode === 'reset') {
        await api.vaultResetPassword(password);
        await session.opened();
      } else {
        await api.vaultChangePassword(current, password);
        session.back();
      }
      toasts.success('Мастер-пароль изменён');
    } catch (error) {
      if (error instanceof ApiError && error.code === 'wrong_password') {
        currentError = errorText(error);
        currentInput?.focus();
      } else {
        reportError(error);
      }
    } finally {
      busy = false;
    }
  }

  function lock() {
    api.vaultLock().catch(reportError);
  }
</script>

<main class="screen">
  <header data-tauri-drag-region>
    {#if mode === 'change'}
      <button type="button" class="ib" aria-label="Назад (Esc)" onclick={() => session.back()}>
        <Icon name="back" />
      </button>
      <h1 data-tauri-drag-region>Смена мастер-пароля</h1>
    {:else}
      <h1 class="big" data-tauri-drag-region>Задайте новый мастер-пароль</h1>
      <button
        type="button"
        class="ib"
        aria-label="Заблокировать"
        title="Заблокировать"
        onclick={lock}
      >
        <Icon name="lock" />
      </button>
    {/if}
  </header>

  <div class="content">
    {#if mode === 'reset'}
      <p class="muted">
        Вы вошли по ключу восстановления. Старый мастер-пароль больше не нужен — придумайте новый.
        Ключ восстановления продолжит работать.
      </p>
    {:else}
      <div>
        <label class="lbl" for="{id}-current">Текущий мастер-пароль</label>
        <PasswordInput
          bind:this={currentInput}
          bind:value={current}
          id="{id}-current"
          autofocus
          invalid={!!currentError}
          describedby={currentError ? `${id}-error` : undefined}
          oninput={() => (currentError = '')}
        />
        {#if currentError}
          <p id="{id}-error" class="field-error" role="alert">{currentError}</p>
        {/if}
      </div>
    {/if}

    <MasterPasswordForm
      bind:password
      bind:valid
      label="Новый мастер-пароль"
      autofocus={mode === 'reset'}
      onsubmit={save}
    />

    <div class="spacer"></div>
    <button type="button" class="btn accent lg" disabled={!ready || busy} onclick={save}>
      {#if busy}Сохраняю…{:else if mode === 'reset'}Сохранить и открыть{:else}Сменить пароль{/if}
    </button>
  </div>
</main>

<style>
  .screen {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 8px 0;
  }

  h1 {
    flex: 1;
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  h1.big {
    padding: 20px 0 0 16px;
    font-family: var(--font-display);
    font-size: var(--text-2xl);
    line-height: 1.25;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 16px;
    min-height: 0;
    padding: 12px 24px 20px;
    overflow-y: auto;
  }

  .spacer {
    flex: 1;
  }
</style>
