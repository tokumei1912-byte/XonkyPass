<script lang="ts">
  import { copyText, reportError } from '../../lib/actions';
  import { api } from '../../lib/api';
  import Icon from '../../lib/ui/Icon.svelte';
  import { toasts } from '../../lib/ui/overlays.svelte';

  interface Props {
    /** Ключ в виде `XXXX-XXXX-…`. */
    key: string;
    doneLabel: string;
    ondone: () => void;
  }

  let { key, doneLabel, ondone }: Props = $props();

  let saved = $state(false);
  const groups = $derived(key.split('-'));
  const id = $props.id();

  async function saveFile() {
    try {
      if (await api.recoveryKeySave(key)) toasts.success('Ключ сохранён в файл');
    } catch (error) {
      reportError(error);
    }
  }
</script>

<div class="card key-card">
  <div class="groups mono selectable" role="group" aria-label="Ключ восстановления: {key}">
    {#each groups as group, i (i)}
      <span class="group">{group}</span>
    {/each}
  </div>
  <div class="actions">
    <button
      type="button"
      class="btn"
      onclick={() => copyText(key, 'Ключ восстановления скопирован')}
    >
      <Icon name="copy" />Копия
    </button>
    <button type="button" class="btn" onclick={saveFile}><Icon name="download" />Файл</button>
    <button type="button" class="btn" onclick={() => window.print()}>
      <Icon name="printer" />Печать
    </button>
  </div>
</div>

<p class="muted small">
  Ключ показывается один раз. Новый можно создать в настройках — старый тогда перестанет работать.
  Регистр и дефисы при вводе не важны.
</p>

<label class="confirm" for="{id}-saved">
  <input id="{id}-saved" type="checkbox" bind:checked={saved} />
  <span>Ключ сохранён в надёжном месте</span>
</label>

<div class="spacer"></div>

<button type="button" class="btn accent lg" disabled={!saved} onclick={ondone}>{doneLabel}</button>

<!-- Лист для печати: остальной интерфейс при печати скрыт. -->
<div class="print-sheet" aria-hidden="true">
  <h1>XonkyPass — ключ восстановления</h1>
  <p class="print-key">{key}</p>
  <p>Ключ открывает хранилище, если вы забудете мастер-пароль.</p>
  <p>Храните его отдельно от компьютера: на бумаге или на флешке в надёжном месте.</p>
</div>

<style>
  .key-card {
    padding: 14px;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
    font-size: 17px;
    font-weight: 600;
    letter-spacing: 0.06em;
  }

  .group {
    padding: 6px 0;
    border-radius: var(--radius);
    background: var(--hover);
    text-align: center;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
    margin-top: 12px;
  }

  .confirm {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
  }

  .confirm input {
    width: 18px;
    height: 18px;
    margin-top: 1px;
    flex: none;
  }

  .spacer {
    flex: 1;
  }

  .print-sheet {
    display: none;
  }

  @media print {
    :global(body *) {
      visibility: hidden;
    }

    .print-sheet,
    .print-sheet * {
      visibility: visible;
    }

    .print-sheet {
      position: fixed;
      inset: 0;
      display: block;
      padding: 48px;
      color: #000000;
      font-family: var(--font-ui);
    }

    .print-sheet h1 {
      margin-bottom: 24px;
      font-size: 20px;
    }

    .print-sheet p {
      margin: 0 0 8px;
      font-size: 14px;
    }

    .print-key {
      margin: 0 0 24px !important;
      font-family: var(--font-mono);
      font-size: 26px !important;
      font-weight: 600;
      letter-spacing: 0.08em;
    }
  }
</style>
