<script lang="ts">
  import type { HTMLInputAttributes } from 'svelte/elements';
  import Icon from './Icon.svelte';

  interface Props {
    value: string;
    id?: string;
    placeholder?: string;
    /** Показан ли пароль. */
    revealed?: boolean;
    invalid?: boolean;
    autofocus?: boolean;
    /** Подсказки про Caps Lock и русскую раскладку — для мастер-пароля. */
    keyboardHints?: boolean;
    /** Кнопка «Новый случайный пароль». */
    ongenerate?: () => void;
    /** Кнопка «Настроить генератор». */
    onopengenerator?: () => void;
    /** Кнопка «Копировать». */
    oncopy?: () => void;
    oninput?: (value: string) => void;
    onkeydown?: HTMLInputAttributes['onkeydown'];
    describedby?: string;
    /** Внутри поле пароля — для автозаполнения в браузере не используется. */
    autocomplete?: HTMLInputAttributes['autocomplete'];
  }

  let {
    value = $bindable(),
    id,
    placeholder,
    revealed = $bindable(false),
    invalid = false,
    autofocus = false,
    keyboardHints = false,
    ongenerate,
    onopengenerator,
    oncopy,
    oninput,
    onkeydown,
    describedby,
    autocomplete = 'off',
  }: Props = $props();

  let input: HTMLInputElement | undefined = $state();
  let capsLock = $state(false);
  let cyrillic = $state(false);

  $effect(() => {
    if (autofocus) input?.focus();
  });

  function trackKeys(e: KeyboardEvent) {
    if (keyboardHints && typeof e.getModifierState === 'function') {
      capsLock = e.getModifierState('CapsLock');
    }
  }

  function onbeforeinput(e: InputEvent) {
    // Последняя набранная буква показывает раскладку, даже когда пароль скрыт.
    const letter = e.data?.match(/\p{L}/u)?.[0];
    if (keyboardHints && letter) cyrillic = /[Ѐ-ӿ]/.test(letter);
  }

  /** Поставить фокус в поле (например, после ошибки). */
  export function focus() {
    input?.focus();
    input?.select();
  }
</script>

<div class="inp" class:invalid>
  <input
    bind:this={input}
    bind:value
    {id}
    type={revealed ? 'text' : 'password'}
    class:mono={revealed}
    {placeholder}
    {autocomplete}
    spellcheck="false"
    autocapitalize="off"
    aria-invalid={invalid}
    aria-describedby={describedby}
    oninput={() => oninput?.(value)}
    onkeydown={(e) => {
      trackKeys(e);
      onkeydown?.(e);
    }}
    onkeyup={trackKeys}
    {onbeforeinput}
    onblur={() => (capsLock = false)}
  />
  <button
    type="button"
    class="ib"
    aria-label={revealed ? 'Скрыть пароль' : 'Показать пароль'}
    title={revealed ? 'Скрыть' : 'Показать'}
    onclick={() => (revealed = !revealed)}
  >
    <Icon name={revealed ? 'eye-off' : 'eye'} />
  </button>
  {#if ongenerate}
    <button
      type="button"
      class="ib"
      aria-label="Новый случайный пароль"
      title="Новый случайный пароль"
      onclick={ongenerate}
    >
      <Icon name="refresh" />
    </button>
  {/if}
  {#if onopengenerator}
    <button
      type="button"
      class="ib"
      aria-label="Настроить генератор"
      title="Настроить генератор"
      onclick={onopengenerator}
    >
      <Icon name="sliders" />
    </button>
  {/if}
  {#if oncopy}
    <button
      type="button"
      class="ib"
      aria-label="Копировать пароль"
      title="Копировать"
      onclick={oncopy}
    >
      <Icon name="copy" />
    </button>
  {/if}
</div>

{#if keyboardHints && (capsLock || cyrillic)}
  <div class="hints" aria-live="polite">
    {#if capsLock}
      <span class="hint warn"><Icon name="alert" size={14} />Включён Caps Lock</span>
    {/if}
    {#if cyrillic}
      <span class="hint"><Icon name="keyboard" size={14} />Раскладка: русская</span>
    {/if}
  </div>
{/if}

<style>
  input.mono {
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .hints {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin-top: 8px;
  }

  .hint {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .hint.warn {
    color: var(--warn);
  }
</style>
