<script lang="ts">
  import { currentWindowLabel } from './lib/window';
  import Panel from './views/Panel.svelte';
  import Palette from './views/Palette.svelte';

  // Один бандл на оба окна: экран выбирается по имени окна Tauri.
  const label = currentWindowLabel();
</script>

{#if label === 'palette'}
  <Palette />
{:else}
  <Panel />
{/if}
