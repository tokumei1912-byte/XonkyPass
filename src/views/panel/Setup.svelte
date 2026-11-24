<script lang="ts">
  import { reportError } from '../../lib/actions';
  import { api } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import MasterPasswordForm from './MasterPasswordForm.svelte';
  import RecoveryKeyView from './RecoveryKeyView.svelte';

  let password = $state('');
  let valid = $state(false);
  let busy = $state(false);
  /** Ключ восстановления — после создания хранилища (шаг 2). */
  let recoveryKey = $state<string | null>(null);

  async function create() {
    if (!valid || busy) return;
    busy = true;
    try {
      recoveryKey = await api.vaultCreate(password);
      password = '';
    } catch (error) {
      reportError(error);
    } finally {
      busy = false;
    }
  }

  function finish() {
    recoveryKey = null;
    void session.opened().catch(reportError);
  }
</script>

<main class="wizard">
  <div class="top" data-tauri-drag-region>
    <span class="tile"><Icon name={recoveryKey ? 'shield-check' : 'key'} size={22} /></span>
    <div class="step" data-tauri-drag-region>
      Шаг {recoveryKey ? 2 : 1} из 2
      <span class="dots" aria-hidden="true"><i class="on"></i><i class:on={recoveryKey}></i></span>
    </div>
  </div>

  {#if recoveryKey}
    <div>
      <h1>Сохраните ключ восстановления</h1>
      <p class="muted">
        Если вы забудете мастер-пароль, этот ключ откроет хранилище. Запишите или распечатайте его и
        храните не на этом компьютере.
      </p>
    </div>
    <RecoveryKeyView key={recoveryKey} doneLabel="Открыть хранилище" ondone={finish} />
  {:else}
    <div>
      <h1>Создайте мастер-пароль</h1>
      <p class="muted">
        Он открывает хранилище. XonkyPass нигде его не сохраняет и не сможет напомнить.
      </p>
    </div>
    <MasterPasswordForm bind:password bind:valid autofocus onsubmit={create} />
    <div class="note warn" role="note">
      <Icon name="alert" size={18} />
      <p>
        Если забыть мастер-пароль и потерять ключ восстановления, данные вернуть нельзя. Ключ вы
        получите на следующем шаге.
      </p>
    </div>
    <div class="spacer"></div>
    <button type="button" class="btn accent lg" disabled={!valid || busy} onclick={create}>
      {busy ? 'Создаю хранилище…' : 'Далее'}
    </button>
  {/if}
</main>

<style>
  .wizard {
    display: flex;
    flex-direction: column;
    gap: 18px;
    height: 100%;
    padding: 28px 24px 20px;
    overflow-y: auto;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
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

  .step {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .dots {
    display: flex;
    gap: 6px;
  }

  .dots i {
    width: 24px;
    height: 4px;
    border-radius: 2px;
    background: var(--border-strong);
  }

  .dots i.on {
    background: var(--accent);
  }

  h1 {
    margin-bottom: 6px;
    font-size: var(--text-2xl);
    font-weight: 600;
    line-height: 1.25;
  }

  .spacer {
    flex: 1;
  }
</style>
