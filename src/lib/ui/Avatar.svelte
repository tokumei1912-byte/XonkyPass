<script lang="ts">
  import type { ItemKind } from '../api';
  import Icon from './Icon.svelte';

  interface Props {
    title: string;
    kind: ItemKind;
    size?: number;
  }

  let { title, kind, size = 32 }: Props = $props();

  const AVATAR_COLORS = 10;

  // Цвет зависит только от названия: у записи он не меняется между запусками.
  const color = $derived.by(() => {
    let hash = 0;
    for (const char of title.toLowerCase()) hash = (hash * 31 + char.codePointAt(0)!) >>> 0;
    return `var(--avatar-${(hash % AVATAR_COLORS) + 1})`;
  });
  const letter = $derived([...title.trim()][0]?.toUpperCase() ?? '?');
</script>

{#if kind === 'note'}
  <span
    class="av note"
    style:width="{size}px"
    style:height="{size}px"
    style:border-radius="{size / 4}px"
  >
    <Icon name="note" size={Math.round(size * 0.55)} />
  </span>
{:else}
  <span
    class="av"
    style:width="{size}px"
    style:height="{size}px"
    style:border-radius="{size / 4}px"
    style:font-size="{Math.round(size * 0.42)}px"
    style:background={color}
    aria-hidden="true"
  >
    {letter}
  </span>
{/if}

<style>
  .av {
    display: grid;
    place-items: center;
    flex: none;
    color: #ffffff;
    font-weight: 600;
    line-height: 1;
  }

  .av.note {
    background: var(--accent-soft);
    color: var(--accent);
  }
</style>
