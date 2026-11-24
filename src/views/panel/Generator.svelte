<script lang="ts">
  import { copyText, reportError } from '../../lib/actions';
  import { api, PASSPHRASE_WORDS, type PasswordOptions, type Strength } from '../../lib/api';
  import { crackTime } from '../../lib/format';
  import {
    generate,
    loadGeneratorOptions,
    saveGeneratorOptions,
    type GeneratorMode,
  } from '../../lib/generator';
  import Icon from '../../lib/ui/Icon.svelte';
  import Segmented from '../../lib/ui/Segmented.svelte';
  import Select from '../../lib/ui/Select.svelte';
  import StrengthMeter from '../../lib/ui/StrengthMeter.svelte';
  import Switch from '../../lib/ui/Switch.svelte';

  interface Props {
    /** Вставить пароль в запись. */
    onuse: (password: string) => void;
    onback: () => void;
  }

  let { onuse, onback }: Props = $props();

  const options = $state(loadGeneratorOptions());
  let value = $state('');
  let strength = $state<Strength | null>(null);
  const id = $props.id();

  const MODES = [
    { value: 'password', label: 'Пароль' },
    { value: 'passphrase', label: 'Парольная фраза' },
  ] as const satisfies readonly { value: GeneratorMode; label: string }[];

  const SETS: readonly { key: keyof PasswordOptions & string; label: string }[] = [
    { key: 'uppercase', label: 'Заглавные A–Z' },
    { key: 'lowercase', label: 'Строчные a–z' },
    { key: 'digits', label: 'Цифры 0–9' },
    { key: 'symbols', label: 'Символы !@#$' },
  ];

  const SEPARATORS = [
    { value: '-', label: 'дефис «-»' },
    { value: ' ', label: 'пробел' },
    { value: '.', label: 'точка «.»' },
    { value: '_', label: 'подчёркивание «_»' },
    { value: '', label: 'без разделителя' },
  ];

  const enabledSets = $derived(SETS.filter((set) => options.password[set.key]).length);

  // Новый вариант при любом изменении настроек; настройки запоминаются.
  $effect(() => {
    const snapshot = $state.snapshot(options);
    saveGeneratorOptions(snapshot);
    void regenerate(snapshot);
  });

  async function regenerate(snapshot = $state.snapshot(options)) {
    try {
      const next = await generate(snapshot);
      value = next;
      strength = await api.passwordStrength(next, []);
    } catch (error) {
      reportError(error);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onback();
    } else if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && value) {
      e.preventDefault();
      onuse(value);
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="generator" {onkeydown}>
  <header data-tauri-drag-region>
    <button type="button" class="ib" aria-label="Назад (Esc)" title="Назад (Esc)" onclick={onback}>
      <Icon name="back" />
    </button>
    <h1 data-tauri-drag-region>Генератор</h1>
  </header>

  <div class="content">
    <Segmented options={MODES} bind:value={options.mode} label="Что сгенерировать" />

    <div class="card result">
      <div class="value-row">
        <output
          class="value mono selectable"
          class:phrase={options.mode === 'passphrase'}
          aria-live="polite"
        >
          {value}
        </output>
        <button
          type="button"
          class="ib"
          aria-label="Другой вариант"
          title="Другой вариант"
          onclick={() => regenerate()}
        >
          <Icon name="refresh" />
        </button>
        <button
          type="button"
          class="ib"
          aria-label="Копировать"
          title="Копировать"
          onclick={() => copyText(value, 'Пароль скопирован')}
        >
          <Icon name="copy" />
        </button>
      </div>
      <div class="meter">
        <StrengthMeter
          {strength}
          detail={strength ? crackTime(strength.guesses_log10) : undefined}
        />
      </div>
    </div>

    {#if options.mode === 'password'}
      <div class="card">
        <div class="opt">
          <label for="{id}-length" class="opt-label">Длина</label>
          <input
            id="{id}-length"
            type="range"
            min="8"
            max="64"
            bind:value={options.password.length}
          />
          <span class="mono num">{options.password.length}</span>
        </div>
        <div class="opt sets">
          {#each SETS as set (set.key)}
            {@const checked = options.password[set.key] as boolean}
            <label class="check">
              <input
                type="checkbox"
                {checked}
                disabled={checked && enabledSets === 1}
                onchange={(e) =>
                  (options.password = { ...options.password, [set.key]: e.currentTarget.checked })}
              />
              {set.label}
            </label>
          {/each}
        </div>
        <div class="opt">
          <div class="grow">
            <div id="{id}-similar">Без похожих символов</div>
            <div class="muted small">не использовать <span class="mono">l 1 I O 0</span></div>
          </div>
          <Switch bind:checked={options.password.avoid_similar} labelledby="{id}-similar" />
        </div>
      </div>
    {:else}
      <div class="card">
        <div class="opt">
          <label for="{id}-words" class="opt-label">Слов</label>
          <input
            id="{id}-words"
            type="range"
            min={PASSPHRASE_WORDS.min}
            max="12"
            bind:value={options.passphrase.words}
          />
          <span class="mono num">{options.passphrase.words}</span>
        </div>
        <div class="opt">
          <label for="{id}-separator" class="grow">Разделитель</label>
          <Select
            id="{id}-separator"
            options={SEPARATORS}
            bind:value={options.passphrase.separator}
          />
        </div>
        <div class="opt sets">
          <label class="check">
            <input type="checkbox" bind:checked={options.passphrase.capitalize} />С заглавной
          </label>
          <label class="check">
            <input type="checkbox" bind:checked={options.passphrase.number} />Добавить цифру
          </label>
        </div>
      </div>
      <p class="muted small">
        Слова берутся из списка EFF (7776 английских слов). Фразу легче запомнить и набрать.
      </p>
    {/if}
  </div>

  <footer>
    <button type="button" class="btn accent" disabled={!value} onclick={() => onuse(value)}>
      Использовать
    </button>
    <button
      type="button"
      class="btn"
      disabled={!value}
      onclick={() => copyText(value, 'Пароль скопирован')}
    >
      Копировать
    </button>
  </footer>
</main>

<style>
  .generator {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px 6px 8px;
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
    padding: 6px 16px 12px;
    overflow-y: auto;
  }

  .result {
    flex: none;
    padding: 14px 8px 12px 14px;
  }

  .value-row {
    display: flex;
    align-items: flex-start;
    gap: 4px;
  }

  .value {
    flex: 1;
    min-width: 0;
    min-height: 52px;
    font-size: 19px;
    line-height: 1.35;
    word-break: break-all;
  }

  .value.phrase {
    font-size: 17px;
    word-break: break-word;
  }

  .meter {
    margin: 12px 6px 0 0;
  }

  .card {
    flex: none;
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 0 12px;
    border-top: 1px solid var(--border);
  }

  .opt:first-child {
    border-top: 0;
  }

  .opt-label {
    width: 52px;
  }

  .opt input[type='range'] {
    flex: 1;
  }

  .num {
    width: 28px;
    font-weight: 600;
    text-align: right;
  }

  .sets {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    padding-top: 12px;
    padding-bottom: 12px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }

  .grow {
    flex: 1;
    min-width: 0;
  }

  footer {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }

  footer .btn {
    flex: 1;
  }
</style>
