<script lang="ts">
  import { api, ApiError, errorText } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import PasswordInput from '../../lib/ui/PasswordInput.svelte';

  let password = $state('');
  let busy = $state(false);
  let error = $state('');
  let input: PasswordInput | undefined = $state();
  const id = $props.id();

  async function unlock() {
    if (!password || busy) return;
    busy = true;
    error = '';
    try {
      await api.vaultUnlock(password);
      password = '';
      await session.opened();
    } catch (e) {
      error = errorText(e);
      if (e instanceof ApiError && e.code === 'wrong_password') input?.focus();
    } finally {
      busy = false;
    }
  }
</script>

<main class="unlock">
  <div class="grow" data-tauri-drag-region></div>
  <div class="brand" data-tauri-drag-region>
    <span class="tile"><Icon name="lock" size={28} /></span>
    <h1>XonkyPass</h1>
    <p class="muted">Хранилище заблокировано</p>
  </div>

  <form
    onsubmit={(e) => {
      e.preventDefault();
      void unlock();
    }}
  >
    <label class="lbl" for="{id}-password">Мастер-пароль</label>
    <PasswordInput
      bind:this={input}
      bind:value={password}
      id="{id}-password"
      autofocus
      keyboardHints
      invalid={!!error}
      describedby={error ? `${id}-error` : undefined}
      oninput={() => (error = '')}
    />
    {#if error}
      <p id="{id}-error" class="field-error" role="alert">{error}</p>
    {/if}
    <button type="submit" class="btn accent unlock-btn" disabled={!password || busy}>
      {busy ? 'Проверяю…' : 'Разблокировать'}
    </button>
    <button type="button" class="recovery" onclick={() => (session.phase = 'recovery')}>
      Забыли пароль? Войти по ключу восстановления
    </button>
  </form>

  <div class="grow more" data-tauri-drag-region></div>
  <p class="keys muted small"><kbd>Enter</kbd> разблокировать · <kbd>Esc</kbd> спрятать</p>
</main>

<style>
  .unlock {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 24px 32px 18px;
  }

  .grow {
    flex: 1;
  }

  .grow.more {
    flex: 1.3;
  }

  .brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    margin-bottom: 28px;
  }

  .tile {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 14px;
    background: var(--accent);
    color: var(--on-accent);
  }

  h1 {
    font-size: var(--text-2xl);
    font-weight: 600;
  }

  form {
    display: flex;
    flex-direction: column;
  }

  .unlock-btn {
    height: 38px;
    margin-top: 16px;
  }

  .recovery {
    align-self: center;
    margin-top: 12px;
    padding: 2px 4px;
    border: 0;
    border-radius: var(--radius-sm);
    background: none;
    color: var(--accent);
    font-size: 13px;
    cursor: pointer;
  }

  .recovery:hover {
    text-decoration: underline;
  }

  .keys {
    text-align: center;
  }
</style>
