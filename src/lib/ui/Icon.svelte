<script lang="ts">
  import { icons, type IconName } from './icons';

  interface Props {
    name: IconName;
    size?: number;
    /** Залить фигуру (например, звезда избранного). */
    filled?: boolean;
    /** Текст для экранного диктора. Без него иконка декоративная. */
    label?: string;
    class?: string;
    style?: string;
  }

  let { name, size = 16, filled = false, label, class: className, style }: Props = $props();
</script>

<svg
  xmlns="http://www.w3.org/2000/svg"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? 'currentColor' : 'none'}
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  class={className}
  {style}
  role={label ? 'img' : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : 'true'}
>
  {#each icons[name] as [tag, attrs], i (i)}
    <svelte:element this={tag} xmlns="http://www.w3.org/2000/svg" {...attrs} />
  {/each}
</svg>

<style>
  svg {
    flex: none;
  }
</style>
