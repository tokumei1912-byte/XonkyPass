<script lang="ts">
  import {
    copyField,
    copyPassword,
    copyUsername,
    itemMenu,
    openSite,
    reportError,
    toggleFavorite,
  } from '../../lib/actions';
  import {
    api,
    type ItemSecret,
    type ItemSummary,
    type LoginMeta,
    type Strength,
  } from '../../lib/api';
  import { formatDate, plural, strengthLabel } from '../../lib/format';
  import { session } from '../../lib/session.svelte';
  import Avatar from '../../lib/ui/Avatar.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { menu } from '../../lib/ui/overlays.svelte';

  interface Props {
    item: ItemSummary;
  }

  let { item }: Props = $props();

  const meta = $derived(item.meta as LoginMeta);
  let secret = $state<ItemSecret | null>(null);
  let strength = $state<Strength | null>(null);
  let showPassword = $state(false);
  /** Индексы показанных скрытых полей. */
  let shownFields = $state<number[]>([]);
  let moreButton: HTMLButtonElement | undefined = $state();

  // Секреты — только пока открыта карточка; после правки (новая ревизия) — заново.
  $effect(() => {
    const { id, revision } = item;
    let alive = true;
    api.itemReveal(id).then((value) => {
      if (alive && revision === item.revision) secret = value;
    }, reportError);
    return () => {
      alive = false;
    };
  });

  $effect(() => {
    const password = secret?.password;
    if (!password) {
      strength = null;
      return;
    }
    const inputs = [meta.title, meta.username, ...meta.urls];
    api.passwordStrength(password, inputs).then(
      (value) => (strength = value),
      () => (strength = null),
    );
  });

  const badge = $derived(strength ? strengthLabel(strength) : null);

  function showMenu() {
    if (moreButton)
      menu.openAt(moreButton, `Действия с записью ${meta.title}`, itemMenu(item, { inCard: true }));
  }

  function openTag(tag: string) {
    session.filter = { kind: 'tag', tag };
    session.query = '';
    session.home();
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.code === 'KeyE') {
      e.preventDefault();
      session.go({ name: 'edit', id: item.id, kind: 'login' });
    } else if (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) {
      e.preventDefault();
      showMenu();
    }
  }

  const mask = (value: string) => '•'.repeat(Math.min(Math.max(value.length, 8), 20));
</script>

<svelte:window {onkeydown} />

<main class="card-screen">
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
    <div class="grow" data-tauri-drag-region></div>
    <button
      type="button"
      class="ib"
      class:on={meta.favorite}
      aria-label={meta.favorite ? 'Убрать из избранного' : 'Добавить в избранное'}
      aria-pressed={meta.favorite}
      title={meta.favorite ? 'Убрать из избранного' : 'В избранное'}
      onclick={() => toggleFavorite(item)}
    >
      <Icon name="star" filled={meta.favorite} />
    </button>
    <button
      type="button"
      class="ib"
      aria-label="Изменить (Ctrl+E)"
      title="Изменить (Ctrl+E)"
      onclick={() => session.go({ name: 'edit', id: item.id, kind: 'login' })}
    >
      <Icon name="pencil" />
    </button>
    <button
      bind:this={moreButton}
      type="button"
      class="ib"
      aria-label="Ещё"
      aria-haspopup="menu"
      title="Ещё"
      onclick={showMenu}
    >
      <Icon name="more" />
    </button>
  </header>

  <div class="content">
    <div class="head">
      <Avatar title={meta.title} kind="login" size={48} />
      <div class="head-text">
        <h1 class="selectable">{meta.title}</h1>
        <div class="muted small">Изменено {formatDate(item.updated_at)}</div>
      </div>
    </div>

    <div class="card fields">
      <div class="field">
        <div class="field-text">
          <div class="key">Логин</div>
          {#if meta.username}
            <div class="value selectable ellipsis">{meta.username}</div>
          {:else}
            <div class="value empty">не указан</div>
          {/if}
        </div>
        {#if meta.username}
          <button
            type="button"
            class="ib"
            aria-label="Копировать логин"
            title="Копировать"
            onclick={() => copyUsername(item)}
          >
            <Icon name="copy" />
          </button>
        {/if}
      </div>

      <div class="field">
        <div class="field-text">
          <div class="key">Пароль</div>
          {#if !secret}
            <div class="value empty">…</div>
          {:else if !secret.password}
            <div class="value empty">не задан</div>
          {:else if showPassword}
            <div class="value mono selectable secret">{secret.password}</div>
          {:else}
            <div class="value mono masked" aria-label="пароль скрыт">{mask(secret.password)}</div>
          {/if}
        </div>
        {#if secret?.password}
          {#if badge}
            <span class="badge {badge.tone}">{badge.label}</span>
          {/if}
          <button
            type="button"
            class="ib"
            aria-label={showPassword ? 'Скрыть пароль' : 'Показать пароль'}
            title={showPassword ? 'Скрыть' : 'Показать'}
            onclick={() => (showPassword = !showPassword)}
          >
            <Icon name={showPassword ? 'eye-off' : 'eye'} />
          </button>
          <button
            type="button"
            class="ib"
            aria-label="Копировать пароль"
            title="Копировать"
            onclick={() => copyPassword(item)}
          >
            <Icon name="copy" />
          </button>
        {/if}
      </div>

      {#if meta.urls.length > 0}
        <div class="field">
          <div class="field-text">
            <div class="key">{meta.urls.length === 1 ? 'Сайт' : 'Сайты'}</div>
            <div class="value links">
              {#each meta.urls as url, i (url)}
                <span class="url">
                  <button
                    type="button"
                    class="link"
                    title="Открыть {url}"
                    onclick={() => openSite(url)}
                  >
                    {url}
                  </button>{#if i < meta.urls.length - 1},{/if}
                </span>
              {/each}
            </div>
          </div>
        </div>
      {/if}

      {#if meta.apps.length > 0}
        <div class="field">
          <div class="field-text">
            <div class="key">{meta.apps.length === 1 ? 'Приложение' : 'Приложения'}</div>
            <div class="value apps">
              {#each meta.apps as app (app)}
                <span class="app"><Icon name="app" size={14} />{app}</span>
              {/each}
            </div>
          </div>
        </div>
      {/if}

      {#each secret?.fields ?? [] as field, i (i)}
        {@const shown = !field.hidden || shownFields.includes(i)}
        <div class="field">
          <div class="field-text">
            <div class="key ellipsis">{field.name}</div>
            {#if !field.value}
              <div class="value empty">пусто</div>
            {:else if shown}
              <div class="value selectable" class:mono={field.hidden}>{field.value}</div>
            {:else}
              <div class="value mono masked">{mask(field.value)}</div>
            {/if}
          </div>
          {#if field.value}
            {#if field.hidden}
              <button
                type="button"
                class="ib"
                aria-label="{shown ? 'Скрыть' : 'Показать'} {field.name}"
                title={shown ? 'Скрыть' : 'Показать'}
                onclick={() =>
                  (shownFields = shown ? shownFields.filter((n) => n !== i) : [...shownFields, i])}
              >
                <Icon name={shown ? 'eye-off' : 'eye'} />
              </button>
            {/if}
            <button
              type="button"
              class="ib"
              aria-label="Копировать {field.name}"
              title="Копировать"
              onclick={() =>
                copyField(item, { field: 'custom', index: i }, `Поле «${field.name}» скопировано`)}
            >
              <Icon name="copy" />
            </button>
          {/if}
        </div>
      {/each}
    </div>

    {#if meta.tags.length > 0}
      <div class="tags">
        {#each meta.tags as tag (tag)}
          <button
            type="button"
            class="chip"
            title="Показать записи с тегом"
            onclick={() => openTag(tag)}
          >
            #{tag}
          </button>
        {/each}
      </div>
    {/if}

    {#if secret?.notes}
      <div class="card notes">
        <div class="key">Заметки</div>
        <p class="selectable">{secret.notes}</p>
      </div>
    {/if}

    <p class="muted small meta-line">
      Создано {formatDate(item.created_at)}{#if item.use_count > 0}
        · скопировано {item.use_count} {plural(item.use_count, 'раз', 'раза', 'раз')}{/if}
    </p>
  </div>

  <footer>
    <button
      type="button"
      class="btn accent"
      disabled={!secret?.password}
      onclick={() => copyPassword(item)}
    >
      <Icon name="key" />Копировать пароль
    </button>
    <button type="button" class="btn" disabled={!meta.username} onclick={() => copyUsername(item)}>
      <Icon name="user" />Копировать логин
    </button>
  </footer>
</main>

<style>
  .card-screen {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 8px 8px 4px;
  }

  .grow {
    flex: 1;
    align-self: stretch;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    padding: 4px 16px 16px;
    overflow-y: auto;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .head-text {
    min-width: 0;
  }

  h1 {
    font-size: var(--text-xl);
    font-weight: 600;
    line-height: 1.25;
    overflow-wrap: anywhere;
  }

  .fields {
    flex: none;
  }

  .field {
    display: flex;
    align-items: center;
    gap: 2px;
    min-height: 52px;
    padding: 6px 6px 6px 12px;
    border-top: 1px solid var(--border);
  }

  .field:first-child {
    border-top: 0;
  }

  .field-text {
    flex: 1;
    min-width: 0;
  }

  .key {
    color: var(--text-2);
    font-size: var(--text-sm);
  }

  .value {
    overflow-wrap: anywhere;
  }

  .value.empty {
    color: var(--text-3);
  }

  .masked {
    letter-spacing: 0.08em;
    overflow: hidden;
    white-space: nowrap;
  }

  .secret {
    font-size: 13px;
  }

  .badge {
    flex: none;
    margin-right: 6px;
    padding: 1px 8px;
    border-radius: 10px;
    background: var(--hover);
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .badge.ok {
    color: var(--ok);
  }

  .badge.warn {
    color: var(--warn);
  }

  .badge.danger {
    color: var(--danger);
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    column-gap: 6px;
  }

  .url {
    min-width: 0;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }

  .apps {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
  }

  .app {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .app :global(svg) {
    color: var(--text-2);
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .notes {
    flex: none;
    padding: 10px 12px;
  }

  .notes p {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .meta-line {
    white-space: normal;
  }

  footer {
    display: flex;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }

  footer .btn {
    flex: 1;
    min-width: 0;
  }
</style>
