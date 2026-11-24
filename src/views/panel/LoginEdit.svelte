<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { reportError } from '../../lib/actions';
  import {
    api,
    emptySecret,
    type CustomField,
    type ItemId,
    type ItemSecret,
    type LoginMeta,
    type Strength,
  } from '../../lib/api';
  import { plural } from '../../lib/format';
  import { generate, loadGeneratorOptions } from '../../lib/generator';
  import { normalizeTag, titleFrom } from '../../lib/items';
  import { session } from '../../lib/session.svelte';
  import Icon from '../../lib/ui/Icon.svelte';
  import { dialogs } from '../../lib/ui/overlays.svelte';
  import PasswordInput from '../../lib/ui/PasswordInput.svelte';
  import StrengthMeter from '../../lib/ui/StrengthMeter.svelte';
  import Switch from '../../lib/ui/Switch.svelte';
  import TokenInput from '../../lib/ui/TokenInput.svelte';
  import Generator from './Generator.svelte';

  interface Props {
    /** `null` — новая запись. */
    id: ItemId | null;
  }

  let { id }: Props = $props();

  // Экран пересоздаётся для каждой записи ({#key} в Panel), так что начальные данные читаются один раз.
  const start = untrack(() => (id ? session.item(id) : undefined));
  const startMeta = start?.meta.kind === 'login' ? start.meta : undefined;

  let title = $state(startMeta?.title ?? '');
  let username = $state(startMeta?.username ?? '');
  let password = $state('');
  let revealed = $state(false);
  let urls = $state<string[]>([...(startMeta?.urls ?? [])]);
  let apps = $state<string[]>([...(startMeta?.apps ?? [])]);
  let tags = $state<string[]>([...(startMeta?.tags ?? [])]);
  let notes = $state('');
  let fields = $state<CustomField[]>([]);
  let favorite = $state(startMeta?.favorite ?? false);

  let loaded = $state(false);
  let saving = $state(false);
  let generatorOpen = $state(false);
  let strength = $state<Strength | null>(null);
  let titleError = $state('');
  let titleInput: HTMLInputElement | undefined = $state();

  /** Секреты до правки: если не менялись, не перешифровываем. */
  let original: ItemSecret = emptySecret();
  /** Состояние формы после загрузки — чтобы понять, есть ли несохранённые правки. */
  let baseline = '';
  const uid = $props.id();

  const snapshot = () =>
    JSON.stringify({ title, username, password, urls, apps, tags, notes, fields, favorite });

  onMount(() => {
    let alive = true;
    void (async () => {
      try {
        if (id) {
          if (!start) {
            session.back();
            return;
          }
          const secret = await api.itemReveal(id);
          if (!alive) return;
          original = secret;
          password = secret.password;
          notes = secret.notes;
          fields = secret.fields.map((field) => ({ ...field }));
        } else {
          // Новой записи сразу предлагаем надёжный пароль.
          password = await generate(loadGeneratorOptions()).catch(() => '');
          if (!alive) return;
          revealed = password !== '';
        }
        baseline = snapshot();
        loaded = true;
        if (!id) titleInput?.focus();
      } catch (error) {
        reportError(error);
        if (alive) session.back();
      }
    })();
    return () => {
      alive = false;
    };
  });

  async function confirmLeave(): Promise<boolean> {
    if (generatorOpen) {
      generatorOpen = false;
      return false;
    }
    if (!loaded || snapshot() === baseline) return true;
    return dialogs.confirm({
      title: 'Не сохранять изменения?',
      message: id ? 'Правки в записи пропадут.' : 'Новая запись не будет создана.',
      confirm: 'Не сохранять',
      cancel: 'Продолжить',
      danger: true,
    });
  }

  $effect(() => {
    session.guard = confirmLeave;
    return () => {
      if (session.guard === confirmLeave) session.guard = null;
    };
  });

  // Оценка надёжности — с небольшой задержкой, пока пароль набирают.
  $effect(() => {
    const value = password;
    const inputs = [title, username, ...urls];
    if (!value) {
      strength = null;
      return;
    }
    const timer = setTimeout(() => {
      api.passwordStrength(value, inputs).then(
        (result) => {
          if (value === password) strength = result;
        },
        () => (strength = null),
      );
    }, 150);
    return () => clearTimeout(timer);
  });

  async function regenerate() {
    try {
      password = await generate(loadGeneratorOptions());
      revealed = true;
    } catch (error) {
      reportError(error);
    }
  }

  async function addField() {
    fields = [...fields, { name: '', value: '', hidden: false }];
    await tick();
    document.getElementById(`${uid}-field-${fields.length - 1}`)?.focus();
  }

  async function save() {
    if (saving || !loaded) return;
    // Недописанный сайт или тег добавляется при потере фокуса.
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
    await tick();

    const name = title.trim() || titleFrom(urls, apps);
    if (!name) {
      titleError = 'Введите название или сайт';
      titleInput?.focus();
      return;
    }
    const meta: LoginMeta = {
      kind: 'login',
      title: name,
      username: username.trim(),
      urls: [...urls],
      apps: [...apps],
      tags: [...tags],
      favorite,
      has_totp: startMeta?.has_totp ?? false,
    };
    const secret: ItemSecret = {
      password,
      totp: original.totp,
      notes,
      fields: fields
        .filter((field) => field.name.trim() || field.value)
        .map((field, i) => ({
          name: field.name.trim() || `Поле ${i + 1}`,
          value: field.value,
          hidden: field.hidden,
        })),
    };

    saving = true;
    try {
      if (id) {
        const changed = JSON.stringify(secret) !== JSON.stringify(original);
        session.upsert(await api.itemUpdate(id, meta, changed ? secret : null));
        session.back();
      } else {
        const summary = await api.itemCreate(meta, secret);
        session.upsert(summary);
        session.selected = summary.id;
        session.replace({ name: 'item', id: summary.id });
      }
    } catch (error) {
      reportError(error);
    } finally {
      saving = false;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || dialogs.current || generatorOpen) return;
    const ctrl = e.ctrlKey || e.metaKey;
    if (ctrl && (e.code === 'KeyS' || e.key === 'Enter')) {
      e.preventDefault();
      void save();
    }
  }
</script>

<svelte:window {onkeydown} />

{#if generatorOpen}
  <Generator
    onback={() => (generatorOpen = false)}
    onuse={(value) => {
      password = value;
      revealed = true;
      generatorOpen = false;
    }}
  />
{:else}
  <main class="edit">
    <header data-tauri-drag-region>
      <button
        type="button"
        class="ib"
        aria-label="Отмена (Esc)"
        title="Отмена (Esc)"
        onclick={() => session.leave()}
      >
        <Icon name="x" />
      </button>
      <h1 data-tauri-drag-region>{id ? 'Изменить запись' : 'Новый пароль'}</h1>
      <button type="button" class="btn accent save" disabled={!loaded || saving} onclick={save}>
        Сохранить
      </button>
    </header>

    <div class="content">
      <div>
        <label class="lbl" for="{uid}-title">Название</label>
        <div class="inp" class:invalid={titleError}>
          <input
            bind:this={titleInput}
            bind:value={title}
            id="{uid}-title"
            placeholder={titleFrom(urls, apps) || 'Например, GitHub'}
            autocomplete="off"
            aria-invalid={titleError !== ''}
            aria-describedby={titleError ? `${uid}-title-error` : undefined}
            oninput={() => (titleError = '')}
          />
        </div>
        {#if titleError}
          <div class="field-error" id="{uid}-title-error">{titleError}</div>
        {/if}
      </div>

      <div>
        <label class="lbl" for="{uid}-user">Логин</label>
        <div class="inp">
          <input
            bind:value={username}
            id="{uid}-user"
            placeholder="почта или имя пользователя"
            autocomplete="off"
            spellcheck="false"
          />
        </div>
      </div>

      <div>
        <label class="lbl" for="{uid}-pass">Пароль</label>
        <PasswordInput
          bind:value={password}
          bind:revealed
          id="{uid}-pass"
          describedby="{uid}-strength"
          ongenerate={regenerate}
          onopengenerator={() => (generatorOpen = true)}
        />
        <div class="strength" id="{uid}-strength">
          <StrengthMeter
            {strength}
            detail={password
              ? `${password.length} ${plural(password.length, 'символ', 'символа', 'символов')}`
              : undefined}
          />
        </div>
      </div>

      <div>
        <span class="lbl">Сайты</span>
        <TokenInput
          bind:values={urls}
          label="Добавить сайт"
          placeholder={urls.length ? '' : 'например, github.com'}
          normalize={(value) => value.trim().replace(/\/+$/, '')}
        />
      </div>

      <div>
        <span class="lbl">Приложения</span>
        <TokenInput
          bind:values={apps}
          label="Добавить приложение"
          placeholder={apps.length ? '' : 'имя .exe'}
          noun="приложение"
        />
      </div>

      <div>
        <span class="lbl">Теги</span>
        <TokenInput
          bind:values={tags}
          label="Добавить тег"
          prefix="#"
          placeholder={tags.length ? '' : 'работа, банки…'}
          suggestions={session.tags}
          normalize={normalizeTag}
        />
      </div>

      <div>
        <label class="lbl" for="{uid}-notes">Заметки</label>
        <textarea bind:value={notes} id="{uid}-notes" class="area" rows="3" spellcheck="false"
        ></textarea>
      </div>

      {#each fields as field, i (i)}
        <div class="custom" role="group" aria-label="Поле {field.name || i + 1}">
          <div class="inp">
            <input
              bind:value={field.name}
              id="{uid}-field-{i}"
              placeholder="Название"
              aria-label="Название поля"
              autocomplete="off"
            />
          </div>
          <div class="inp">
            <input
              bind:value={field.value}
              type={field.hidden ? 'password' : 'text'}
              class:mono={field.hidden}
              placeholder="Значение"
              aria-label="Значение поля {field.name}"
              autocomplete="off"
              spellcheck="false"
            />
            <button
              type="button"
              class="ib"
              class:on={field.hidden}
              aria-pressed={field.hidden}
              aria-label="Скрытое поле"
              title={field.hidden
                ? 'Скрытое: в карточке показывается по кнопке'
                : 'Сделать скрытым'}
              onclick={() => (field.hidden = !field.hidden)}
            >
              <Icon name={field.hidden ? 'eye-off' : 'eye'} />
            </button>
            <button
              type="button"
              class="ib"
              aria-label="Удалить поле {field.name}"
              title="Удалить поле"
              onclick={() => (fields = fields.filter((_, n) => n !== i))}
            >
              <Icon name="trash" />
            </button>
          </div>
        </div>
      {/each}

      <div class="extras">
        <button type="button" class="btn ghost add" onclick={addField}>
          <Icon name="plus" size={14} />Поле
        </button>
        <div class="grow"></div>
        <span id="{uid}-fav">В избранном</span>
        <Switch bind:checked={favorite} labelledby="{uid}-fav" />
      </div>
    </div>

    <footer class="muted small">
      Ctrl+S — сохранить · Esc — отмена · пустое название возьмётся из сайта
    </footer>
  </main>
{/if}

<style>
  .edit {
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

  .save {
    height: 30px;
  }

  .content {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    padding: 6px 16px 12px;
    overflow-y: auto;
  }

  .strength {
    margin-top: 6px;
  }

  .custom {
    display: grid;
    grid-template-columns: 112px minmax(0, 1fr);
    gap: 6px;
  }

  .extras {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }

  .add {
    padding: 0 8px;
    color: var(--accent);
  }

  .grow {
    flex: 1;
  }

  footer {
    padding: 8px 16px;
    border-top: 1px solid var(--border);
  }
</style>
