<script lang="ts">
  import Icon from './Icon.svelte';
  import { menu, type MenuEntry } from './overlays.svelte';

  let box: HTMLDivElement | undefined = $state();
  let position = $state({ x: 0, y: 0 });
  let active = $state(-1);

  const current = $derived(menu.current);
  const actionable = $derived(
    current
      ? current.entries.flatMap((entry, i) => (entry !== 'separator' && !entry.disabled ? [i] : []))
      : [],
  );

  // Меню не вылезает за край окна: если не помещается снизу, открывается вверх.
  $effect(() => {
    if (!current || !box) return;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const { width, height } = box.getBoundingClientRect();
    const pad = 8;
    position = {
      x: Math.max(pad, Math.min(current.x, window.innerWidth - width - pad)),
      y:
        current.y + height > window.innerHeight - pad
          ? Math.max(pad, current.y - height)
          : current.y,
    };
    active = actionable[0] ?? -1;
    box.focus();
    return () => previous?.focus();
  });

  function run(entry: MenuEntry) {
    if (entry === 'separator' || entry.disabled) return;
    menu.close();
    // Действие — после того, как меню вернуло фокус на место.
    queueMicrotask(() => void entry.run());
  }

  function move(step: number) {
    if (actionable.length === 0) return;
    const index = actionable.indexOf(active);
    active = actionable[(index + step + actionable.length) % actionable.length]!;
  }

  function onkeydown(e: KeyboardEvent) {
    const entry = current?.entries[active];
    switch (e.key) {
      case 'ArrowDown':
        move(1);
        break;
      case 'ArrowUp':
        move(-1);
        break;
      case 'Home':
        active = actionable[0] ?? -1;
        break;
      case 'End':
        active = actionable[actionable.length - 1] ?? -1;
        break;
      case 'Enter':
      case ' ':
        if (entry) run(entry);
        break;
      case 'Escape':
      case 'Tab':
        menu.close();
        break;
      default:
        return;
    }
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onblur={() => menu.close()} onresize={() => menu.close()} />

{#if current}
  <!-- Невидимый слой: щелчок мимо меню закрывает его. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="catcher"
    onmousedown={() => menu.close()}
    oncontextmenu={(e) => {
      e.preventDefault();
      menu.close();
    }}
  ></div>
  <div
    bind:this={box}
    class="menu popup"
    role="menu"
    aria-label={current.label}
    tabindex="-1"
    style:left="{position.x}px"
    style:top="{position.y}px"
    aria-activedescendant={active >= 0 ? `menu-item-${active}` : undefined}
    {onkeydown}
  >
    {#each current.entries as entry, i (i)}
      {#if entry === 'separator'}
        <div class="separator" role="separator"></div>
      {:else}
        <button
          type="button"
          id="menu-item-{i}"
          role={entry.checked === undefined ? 'menuitem' : 'menuitemradio'}
          aria-checked={entry.checked}
          class="mi"
          class:active={i === active}
          class:danger={entry.danger}
          disabled={entry.disabled}
          tabindex="-1"
          onclick={() => run(entry)}
          onmouseenter={() => (active = i)}
        >
          {#if entry.checked !== undefined}
            <span class="check">
              {#if entry.checked}<Icon name="check" size={16} />{/if}
            </span>
          {/if}
          {#if entry.icon}<Icon name={entry.icon} />{/if}
          <span class="ellipsis">{entry.label}</span>
          {#if entry.shortcut}<span class="shortcut">{entry.shortcut}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .catcher {
    position: fixed;
    inset: 0;
    z-index: 30;
  }

  .menu {
    position: fixed;
    z-index: 31;
    width: 252px;
    max-width: calc(100vw - 16px);
    padding: 4px;
    outline: 0;
  }

  .check {
    display: inline-flex;
    flex: none;
    width: 16px;
    color: var(--accent);
  }

  .separator {
    height: 1px;
    margin: 4px 0;
    background: var(--border);
  }
</style>
