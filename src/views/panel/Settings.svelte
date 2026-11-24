<script lang="ts">
  import { onMount } from 'svelte';
  import { reportError } from '../../lib/actions';
  import {
    api,
    type AppInfo,
    type DockMode,
    type DockSettings,
    type MonitorInfo,
    type Settings,
    type Theme,
    type UpdateCheck,
  } from '../../lib/api';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import Select from '../../lib/ui/Select.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import { dialogs, toasts } from '../../lib/ui/overlays.svelte';

  let settings = $state<Settings | null>(null);
  let info = $state<AppInfo | null>(null);
  let update = $state<UpdateCheck | null>(null);
  let checking = $state(false);
  let monitors = $state<MonitorInfo[]>([]);
  const uid = $props.id();

  const MODES: readonly { value: DockMode; label: string }[] = [
    { value: 'left', label: 'Слева' },
    { value: 'right', label: 'Справа' },
    { value: 'top', label: 'Сверху' },
    { value: 'window', label: 'Обычное окно' },
  ];
  const SIDE_WIDTHS = [320, 360, 400, 480, 560, 640];
  const TOP_SIZES: readonly (readonly [number, number])[] = [
    [640, 440],
    [720, 520],
    [840, 600],
    [960, 680],
  ];
  const HOVER_DELAYS = [100, 200, 400, 700];

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
    api.dockMonitors().then((value) => (monitors = value), reportError);
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

  function changeDock(patch: Partial<DockSettings>) {
    if (settings) void change({ dock: { ...settings.dock, ...patch } });
  }

  const dockSettings = $derived(settings?.dock);
  const docked = $derived(dockSettings !== undefined && dockSettings.mode !== 'window');

  /** Экран панели; если сохранённого нет среди подключённых — основной, как в Rust. */
  const monitor = $derived(
    monitors.find((m) => m.id === dockSettings?.monitor) ??
      monitors.find((m) => m.primary) ??
      monitors[0],
  );
  const monitorOptions = $derived(
    monitors.map((m) => ({
      value: m.id,
      label: `Экран ${m.number}${m.primary ? ' · основной' : ''}`,
    })),
  );

  /** Варианты плюс текущее значение, если его задали мышью и его нет в списке. */
  function withCurrent(values: readonly number[], current: number | null | undefined): number[] {
    const all = [...values];
    if (current != null && !all.includes(current)) all.push(current);
    return all.sort((a, b) => a - b);
  }

  const widthOptions = $derived(
    withCurrent(SIDE_WIDTHS, dockSettings?.side_width).map((w) => ({
      value: w,
      label: `${w} px`,
    })),
  );

  const topSizeOptions = $derived.by(() => {
    const sizes = TOP_SIZES.map(([w, h]) => [w, h] as const);
    if (dockSettings) {
      const { top_width: w, top_height: h } = dockSettings;
      if (!sizes.some(([a, b]) => a === w && b === h)) sizes.push([w, h]);
    }
    sizes.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
    return sizes.map(([w, h]) => ({ value: `${w}x${h}`, label: `${w} × ${h}` }));
  });

  function changeTopSize(value: string) {
    const [top_width, top_height] = value.split('x').map(Number);
    if (top_width && top_height) changeDock({ top_width, top_height });
  }

  /** `0` в списке — «не выезжать» (`null` в настройках). */
  const hoverOptions = $derived([
    ...withCurrent(HOVER_DELAYS, dockSettings?.hover_delay_ms).map((ms) => ({
      value: ms,
      label: `через ${(ms / 1000).toLocaleString('ru-RU')} с`,
    })),
    { value: 0, label: 'не выезжать' },
  ]);

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
        <div class="text">
          <div class="tt" id="{uid}-clear">Очищать через</div>
          <div class="dd2">если в буфере всё ещё наш пароль</div>
        </div>
        <Select
          options={CLEAR_OPTIONS}
          value={settings?.clipboard_clear_secs}
          labelledby="{uid}-clear"
          disabled={!settings}
          onchange={(secs) => change({ clipboard_clear_secs: secs })}
        />
      </div>
      <div class="set">
        <Icon name="eye-off" size={18} />
        <div class="text">
          <div class="tt">Не попадает в журнал</div>
          <div class="dd2 wrap">Скопированное не видно в Win+V и не уходит в облачный буфер</div>
        </div>
      </div>
    </div>

    <h2 class="sec">Панель</h2>
    <div class="card">
      <div class="set">
        <Icon name="panel-left" size={18} />
        <div class="text">
          <div class="tt" id="{uid}-mode">Положение</div>
          <div class="dd2 wrap">
            {docked
              ? 'Перетащите за заголовок к другому краю'
              : 'Перетащите к краю экрана, чтобы прикрепить'}
          </div>
        </div>
        <Select
          options={MODES}
          value={dockSettings?.mode}
          labelledby="{uid}-mode"
          disabled={!settings}
          onchange={(mode) => changeDock({ mode })}
        />
      </div>
      {#if docked && dockSettings}
        {#if monitors.length > 1 && monitor}
          <div class="set">
            <Icon name="monitor" size={18} />
            <div class="text">
              <div class="tt" id="{uid}-monitor">Экран</div>
              <div class="dd2">
                {monitor.width} × {monitor.height} · {monitor.scale}%
              </div>
            </div>
            <Select
              options={monitorOptions}
              value={monitor.id}
              labelledby="{uid}-monitor"
              onchange={(id) =>
                changeDock({ monitor: monitors.find((m) => m.id === id)?.primary ? null : id })}
            />
          </div>
        {/if}
        <div class="set">
          <Icon name="move-horizontal" size={18} />
          <div class="text">
            <div class="tt" id="{uid}-size">
              {dockSettings.mode === 'top' ? 'Размер' : 'Ширина'}
            </div>
            <div class="dd2">Или потяните за край панели</div>
          </div>
          {#if dockSettings.mode === 'top'}
            <Select
              options={topSizeOptions}
              value="{dockSettings.top_width}x{dockSettings.top_height}"
              labelledby="{uid}-size"
              onchange={changeTopSize}
            />
          {:else}
            <Select
              options={widthOptions}
              value={dockSettings.side_width}
              labelledby="{uid}-size"
              onchange={(side_width) => changeDock({ side_width })}
            />
          {/if}
        </div>
        <div class="set">
          <Icon name="mouse-pointer" size={18} />
          <div class="text">
            <div class="tt" id="{uid}-hover">Выезжать при наведении</div>
            <div class="dd2">когда мышь у края экрана</div>
          </div>
          <Select
            options={hoverOptions}
            value={dockSettings.hover_delay_ms ?? 0}
            labelledby="{uid}-hover"
            onchange={(ms) => changeDock({ hover_delay_ms: ms === 0 ? null : ms })}
          />
        </div>
        <div class="set">
          <Icon name="chevrons-right" size={18} />
          <div class="text">
            <div class="tt" id="{uid}-animate">Плавно выезжать</div>
            <div class="dd2">и плавно прятаться</div>
          </div>
          <Switch
            checked={dockSettings.animate}
            labelledby="{uid}-animate"
            onchange={(animate) => changeDock({ animate })}
          />
        </div>
        <div class="set">
          <Icon name="panel-left-dashed" size={18} />
          <div class="text">
            <div class="tt" id="{uid}-indicator">Полоска у края</div>
            <div class="dd2 wrap">Тонкая цветная полоса там, где спрятана панель</div>
          </div>
          <Switch
            checked={dockSettings.indicator}
            labelledby="{uid}-indicator"
            onchange={(indicator) => changeDock({ indicator })}
          />
        </div>
      {/if}
    </div>

    <h2 class="sec">Внешний вид</h2>
    <div class="card">
      <div class="set">
        <Icon name="contrast" size={18} />
        <div class="text tt" id="{uid}-theme">Тема</div>
        <Select
          options={THEMES}
          value={settings?.theme}
          labelledby="{uid}-theme"
          disabled={!settings}
          onchange={(theme) => change({ theme })}
        />
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
