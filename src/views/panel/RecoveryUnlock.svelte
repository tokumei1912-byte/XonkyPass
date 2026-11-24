<script lang="ts">
  import { api, errorText } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';

  let key = $state('');
  let busy = $state(false);
  let error = $state('');
  let input: HTMLInputElement | undefined = $state();
  const id = $props.id();

  $effect(() => {
    input?.focus();
  });

  async function open() {
    if (!key.trim() || busy) return;
    busy = true;
    error = '';
    try {
      await api.vaultUnlockRecovery(key);
      key = '';
      session.phase = 'reset';
    } catch (e) {
      error = errorText(e);
      input?.select();
    } finally {
      busy = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      session.phase = 'locked';
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="recovery" {onkeydown}>
  <header data-tauri-drag-region>
    <button
      type="button"
      class="ib"
      aria-label="Назад (Esc)"
      onclick={() => (session.phase = 'locked')}
    >
      <Icon name="back" />
    </button>
  </header>

  <div class="content">
    <span class="tile"><Icon name="shield" size={22} /></span>
    <div>
      <h1>Вход по ключу восстановления</h1>
      <p class="muted">
        Введите ключ, который вы сохранили при создании хранилища. После входа нужно будет задать
        новый мастер-пароль.
      </p>
    </div>

    <form
      onsubmit={(e) => {
        e.preventDefault();
        void open();
      }}
    >
      <label class="lbl" for="{id}-key">Ключ восстановления</label>
      <div class="inp" class:invalid={!!error}>
        <input
          bind:this={input}
          bind:value={key}
          id="{id}-key"
          class="mono key"
          placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
          autocomplete="off"
          spellcheck="false"
          aria-invalid={!!error}
          aria-describedby={error ? `${id}-error` : `${id}-hint`}
          oninput={() => (error = '')}
        />
      </div>
      {#if error}
        <p id="{id}-error" class="field-error" role="alert">{error}</p>
      {:else}
        <p id="{id}-hint" class="field-hint">Регистр, пробелы и дефисы не важны.</p>
      {/if}
      <button type="submit" class="btn accent lg submit" disabled={!key.trim() || busy}>
        {busy ? 'Проверяю…' : 'Открыть хранилище'}
      </button>
    </form>
  </div>
</main>

<style>
  .recovery {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    padding: 8px 8px 0;
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 8px 24px 20px;
  }

  .tile {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 10px;
    background: var(--accent);
    color: var(--on-accent);
  }

  h1 {
    margin-bottom: 6px;
    font-size: var(--text-2xl);
    font-weight: 600;
    line-height: 1.25;
  }

  form {
    display: flex;
    flex-direction: column;
  }

  .key {
    font-size: 13px;
    text-transform: uppercase;
  }

  .key::placeholder {
    text-transform: none;
  }

  .submit {
    margin-top: 16px;
  }
</style>
