<script lang="ts">
  import { api, type Strength } from '../../lib/api';
  import { crackTime } from '../../lib/format';
  import Icon from '../../lib/ui/Icon.svelte';
  import PasswordInput from '../../lib/ui/PasswordInput.svelte';
  import StrengthMeter from '../../lib/ui/StrengthMeter.svelte';

  interface Props {
    password: string;
    /** Пароль достаточно надёжный и повторён без ошибок. */
    valid: boolean;
    label?: string;
    autofocus?: boolean;
    /** Enter во втором поле. */
    onsubmit: () => void;
  }

  let {
    password = $bindable(),
    valid = $bindable(),
    label = 'Мастер-пароль',
    autofocus = false,
    onsubmit,
  }: Props = $props();

  /** Минимум: «средний» по zxcvbn — около 10⁸ попыток, с Argon2id это годы перебора. */
  const MIN_SCORE = 2;

  let repeat = $state('');
  let strength = $state<Strength | null>(null);
  const id = $props.id();

  // Оценка в Rust (zxcvbn) с небольшой задержкой, чтобы не считать на каждую букву.
  $effect(() => {
    const value = password;
    if (!value) {
      strength = null;
      return;
    }
    const timer = setTimeout(() => {
      api.passwordStrength(value, []).then(
        (result) => {
          if (value === password) strength = result;
        },
        () => (strength = null),
      );
    }, 150);
    return () => clearTimeout(timer);
  });

  const matches = $derived(repeat.length > 0 && repeat === password);
  const mismatch = $derived(repeat.length > 0 && !password.startsWith(repeat));
  const weak = $derived(strength !== null && strength.score < MIN_SCORE);

  $effect(() => {
    valid = strength !== null && strength.score >= MIN_SCORE && matches;
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      onsubmit();
    }
  }
</script>

<div>
  <label class="lbl" for="{id}-new">{label}</label>
  <PasswordInput
    id="{id}-new"
    bind:value={password}
    {autofocus}
    keyboardHints
    describedby="{id}-tip"
    onkeydown={(e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        document.getElementById(`${id}-repeat`)?.focus();
      }
    }}
  />
  <div class="meter"><StrengthMeter {strength} /></div>
  <p id="{id}-tip" class="field-hint">
    {#if weak}
      <span class="weak">Слишком простой пароль — его легко подобрать.</span>
    {:else if strength}
      {crackTime(strength.guesses_log10).replace(/^./, (c) => c.toUpperCase())}.
    {/if}
    Совет: 4–5 случайных слов надёжнее короткого сложного пароля и легче запоминаются.
  </p>
</div>

<div>
  <label class="lbl" for="{id}-repeat">Повторите пароль</label>
  <div class="inp" class:invalid={mismatch}>
    <input
      id="{id}-repeat"
      type="password"
      bind:value={repeat}
      autocomplete="off"
      spellcheck="false"
      aria-invalid={mismatch}
      aria-describedby={mismatch ? `${id}-mismatch` : undefined}
      {onkeydown}
    />
    {#if matches}
      <span class="ok"><Icon name="check" label="Пароли совпадают" /></span>
    {/if}
  </div>
  {#if mismatch}
    <p id="{id}-mismatch" class="field-error">Пароли не совпадают</p>
  {/if}
</div>

<style>
  .meter {
    margin-top: 8px;
  }

  .weak {
    color: var(--danger);
  }

  .ok {
    display: grid;
    place-items: center;
    width: 32px;
    color: var(--ok);
  }
</style>
