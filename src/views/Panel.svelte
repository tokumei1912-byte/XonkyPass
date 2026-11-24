<script lang="ts">
  import { onMount } from 'svelte';
  import { reportError } from '../lib/actions';
  import { api } from '../lib/api';
  import { session } from '../lib/session.svelte';
  import ContextMenu from '../lib/ui/ContextMenu.svelte';
  import DialogHost from '../lib/ui/DialogHost.svelte';
  import { dialogs, menu, toasts } from '../lib/ui/overlays.svelte';
  import Toasts from '../lib/ui/Toasts.svelte';
  import { hideCurrentWindow } from '../lib/window';
  import ItemView from './panel/ItemView.svelte';
  import List from './panel/List.svelte';
  import LoginEdit from './panel/LoginEdit.svelte';
  import NewMasterPassword from './panel/NewMasterPassword.svelte';
  import NewRecoveryKey from './panel/NewRecoveryKey.svelte';
  import NoteEdit from './panel/NoteEdit.svelte';
  import NoteView from './panel/NoteView.svelte';
  import RecoveryUnlock from './panel/RecoveryUnlock.svelte';
  import Settings from './panel/Settings.svelte';
  import Setup from './panel/Setup.svelte';
  import Tags from './panel/Tags.svelte';
  import Trash from './panel/Trash.svelte';
  import Unlock from './panel/Unlock.svelte';

  onMount(() => {
    void session.start();
    const subscriptions = [
      api.onVaultLocked(() => session.locked()),
      api.onClipboardCleared(() => toasts.clipboardCleared()),
    ];
    return () => {
      for (const subscription of subscriptions)
        void subscription.then((unsubscribe) => unsubscribe());
    };
  });

  const screen = $derived(session.screen);
  const editing = $derived(
    screen.name === 'edit' || screen.name === 'change-password' || screen.name === 'recovery-key',
  );
  /** Тосты — над нижней панелью экрана, если она есть. */
  const toastOffset = $derived(
    session.phase === 'open' &&
      (screen.name === 'list' || screen.name === 'item') &&
      session.items.length > 0
      ? 64
      : 12,
  );

  // Запись, открытую в карточке, удалили (например, из корзины) — вернуться назад.
  $effect(() => {
    if (session.phase === 'open' && screen.name === 'item' && !session.item(screen.id))
      session.back();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.defaultPrevented || dialogs.current || menu.current) return;
    const ctrl = e.ctrlKey || e.metaKey;

    if (e.key === 'Escape') {
      e.preventDefault();
      if (session.phase === 'open' && session.stack.length > 1) void session.leave();
      else void hideCurrentWindow();
      return;
    }
    if (session.phase !== 'open') return;

    // Сочетания — по физической клавише (`code`), чтобы работали и в русской раскладке.
    if (ctrl && e.code === 'KeyL') {
      e.preventDefault();
      api.vaultLock().catch(reportError);
    } else if (ctrl && e.code === 'KeyN' && !editing) {
      e.preventDefault();
      session.go({ name: 'edit', id: null, kind: e.shiftKey ? 'note' : 'login' });
    } else if (ctrl && e.code === 'KeyF' && screen.name !== 'list' && !editing) {
      e.preventDefault();
      session.home();
    }
  }
</script>

<svelte:window {onkeydown} />

<div class="panel">
  {#if session.phase === 'loading'}
    <div class="center" data-tauri-drag-region></div>
  {:else if session.phase === 'failed'}
    <div class="center failed" data-tauri-drag-region>
      <h1>Не удалось открыть XonkyPass</h1>
      <p class="muted selectable">{session.error}</p>
      <button type="button" class="btn" onclick={() => session.start()}>Повторить</button>
    </div>
  {:else if session.phase === 'setup'}
    <Setup />
  {:else if session.phase === 'locked'}
    <Unlock />
  {:else if session.phase === 'recovery'}
    <RecoveryUnlock />
  {:else if session.phase === 'reset'}
    <NewMasterPassword mode="reset" />
  {:else if screen.name === 'list'}
    <List />
  {:else if screen.name === 'item'}
    {@const item = session.item(screen.id)}
    {#if item}
      {#key item.id}
        {#if item.meta.kind === 'note'}
          <NoteView {item} />
        {:else}
          <ItemView {item} />
        {/if}
      {/key}
    {/if}
  {:else if screen.name === 'edit'}
    {#key screen.id ?? `new-${screen.kind}`}
      {#if screen.kind === 'note'}
        <NoteEdit id={screen.id} />
      {:else}
        <LoginEdit id={screen.id} />
      {/if}
    {/key}
  {:else if screen.name === 'trash'}
    <Trash />
  {:else if screen.name === 'tags'}
    <Tags />
  {:else if screen.name === 'settings'}
    <Settings />
  {:else if screen.name === 'change-password'}
    <NewMasterPassword mode="change" />
  {:else if screen.name === 'recovery-key'}
    <NewRecoveryKey key={screen.key} />
  {/if}

  <Toasts bottom={toastOffset} />
  <DialogHost />
  <ContextMenu />
</div>

<style>
  .panel {
    position: relative;
    height: 100%;
    overflow: hidden;
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 100%;
    padding: 32px;
    text-align: center;
  }

  .failed h1 {
    font-size: 18px;
    font-weight: 600;
  }
</style>
