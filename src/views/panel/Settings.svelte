<script lang="ts">
  import { onMount } from 'svelte';
  import { reportError } from '../../lib/actions';
  import { api, type AppInfo, type Settings, type Theme, type UpdateCheck } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { dialogs, toasts } from '../../lib/ui/overlays.svelte';

  let settings = $state<Settings | null>(null);
  let info = $state<AppInfo | null>(null);
  let update = $state<UpdateCheck | null>(null);
  let checking = $state(false);
  const uid = $props.id();

  const CLEAR_OPTIONS = [
    { value: 10, label: '10 секунд' },
    { value: 20, label: '20 секунд' },
    { value: 30, label: '30 секунд' },
    { value: 60, label: '1 минуту' },
    { value: 120, label: '2 минуты' },
    { value: 0, label: 'не очищать' },
  ];

  const THEMES: readonly { value: Theme; label: string }[] = [
    { value: 'system', label: 'Как в Windows' },
    { value: 'light', label: 'Светлая' },
    { value: 'dark', label: 'Тёмная' },
  ];

  onMount(() => {
    api.settingsGet().then((value) => (settings = value), reportError);
    api.appInfo().then((value) => (info = value), reportError);
    const unsubscribe = api.onSettingsChanged((value) => (settings = value));
    return () => void unsubscribe.then((off) => off());
  });

  async function change(patch: Partial<Settings>) {
    if (!settings) return;
    try {
      settings = await api.settingsSet({ ...settings, ...patch });
    } catch (error) {
      reportError(error);
    }
  }

  async function newRecoveryKey() {
    const ok = await dialogs.confirm({
      title: 'Создать новый ключ восстановления?',
      message: 'Старый ключ сразу перестанет действовать. Новый нужно будет сохранить.',
      confirm: 'Создать',
    });
    if (!ok) return;
    try {
      const key = await api.vaultRegenerateRecoveryKey();
      session.go({ name: 'recovery-key', key });
    } catch (error) {
      reportError(error);
    }
  }

  async function checkUpdate() {
    checking = true;
    try {
      update = await api.updateCheck();
      if (update.status === 'up_to_date') toasts.success('Установлена последняя версия');
      else if (update.status === 'busy')
        toasts.info('Обновление уже проверяется', 'Попробуйте через минуту');
    } catch (error) {
      reportError(error);
    } finally {
      checking = false;
    }
  }

  async function installUpdate(version: string) {
    const ok = await dialogs.confirm({
      title: `Установить версию ${version}?`,
      message: 'XonkyPass закроется и запустится снова. Хранилище нужно будет открыть заново.',
      confirm: 'Установить',
    });
    if (!ok) return;
    try {
      await api.updateInstall();
    } catch (error) {
      reportError(error);
    }
  }

  function revealVault() {
    api.revealVault().catch(reportError);
  }
</script>

<main class="settings">
  <header data-tauri-drag-region>
    <button
      type="button"
      class="ib"
      aria-label="Назад (Esc)"
      title="Назад (Esc)"
      onclick={() => session.back()}
    >
      <Icon name="back" />
    </button>
    <h1 data-tauri-drag-region>Настройки</h1>
  </header>

  <div class="content">
    <h2 class="sec">Безопасность</h2>
    <div class="card">
      <div class="set">
        <Icon name="shield" size={18} />
        <div class="text">
          <div class="tt">Мастер-пароль</div>
          <div class="dd2">Открывает хранилище</div>
        </div>
        <button type="button" class="btn" onclick={() => session.go({ name: 'change-password' })}>
          Сменить
        </button>
      </div>
      <div class="set">
        <Icon name="key" size={18} />
        <div class="text">
          <div class="tt">Ключ восстановления</div>
          <div class="dd2">Новый ключ заменит старый</div>
        </div>
        <button type="button" class="btn" onclick={newRecoveryKey}>Создать новый</button>
      </div>
    </div>

    <h2 class="sec">Буфер обмена</h2>
    <div class="card">
      <div class="set">
        <Icon name="timer" size={18} />
        <label class="text" for="{uid}-clear">
          <span class="tt">Очищать через</span>
          <span class="dd2">если в буфере всё ещё наш пароль</span>
        </label>
        <select
          id="{uid}-clear"
          class="dd"
          disabled={!settings}
          value={settings?.clipboard_clear_secs}
          onchange={(e) => change({ clipboard_clear_secs: Number(e.currentTarget.value) })}
        >
          {#each CLEAR_OPTIONS as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
      </div>
      <div class="set">
        <Icon name="eye-off" size={18} />
        <div class="text">
          <div class="tt">Не попадает в журнал</div>
          <div class="dd2 wrap">Скопированное не видно в Win+V и не уходит в облачный буфер</div>
        </div>
      </div>
    </div>

    <h2 class="sec">Внешний вид</h2>
    <div class="card">
      <div class="set">
        <Icon name="contrast" size={18} />
        <label class="text tt" for="{uid}-theme">Тема</label>
        <select
          id="{uid}-theme"
          class="dd"
          disabled={!settings}
          value={settings?.theme}
          onchange={(e) => change({ theme: e.currentTarget.value as Theme })}
        >
          {#each THEMES as theme (theme.value)}
            <option value={theme.value}>{theme.label}</option>
          {/each}
        </select>
      </div>
    </div>

    <h2 class="sec">О программе</h2>
    <div class="card">
      <div class="set">
        <Icon name="refresh" size={18} />
        <div class="text">
          <div class="tt">XonkyPass {info?.version ?? ''}</div>
          <div class="dd2">
            {#if !info}
              &nbsp;
            {:else if !info.auto_update}
              Сборка для разработки: без обновлений
            {:else if update?.status === 'ready'}
              Готова версия {update.version}
            {:else}
              Обновления ставятся автоматически
            {/if}
          </div>
        </div>
        {#if info?.auto_update}
          {#if update?.status === 'ready'}
            {@const version = update.version}
            <button type="button" class="btn accent" onclick={() => installUpdate(version)}>
              Установить
            </button>
          {:else}
            <button type="button" class="btn" disabled={checking} onclick={checkUpdate}>
              {checking ? 'Проверка…' : 'Проверить'}
            </button>
          {/if}
        {/if}
      </div>
      <div class="set">
        <Icon name="database" size={18} />
        <div class="text">
          <div class="tt">Файл хранилища</div>
          <div class="dd2 mono ellipsis" title={session.vaultPath}>{session.vaultPath}</div>
        </div>
        <button type="button" class="btn" onclick={revealVault}>Показать</button>
      </div>
    </div>
  </div>
</main>

<style>
  .settings {
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
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .content {
    flex: 1;
    min-height: 0;
    padding: 0 12px 12px;
    overflow-y: auto;
  }

  .sec {
    margin: 16px 4px 6px;
    color: var(--text-2);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .set {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 56px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
  }

  .set:first-child {
    border-top: 0;
  }

  .set > :global(svg) {
    flex: none;
    color: var(--text-2);
  }

  .text {
    display: block;
    flex: 1;
    min-width: 0;
  }

  .tt {
    display: block;
  }

  .dd2 {
    display: block;
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .dd2.wrap {
    white-space: normal;
  }

  .mono.dd2 {
    font-size: 11.5px;
  }
</style>
