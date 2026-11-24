<script lang="ts">
  interface Props {
    checked: boolean;
    /** id элемента с подписью. */
    labelledby?: string;
    label?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(), labelledby, label, disabled = false, onchange }: Props = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class="sw"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-labelledby={labelledby}
  aria-label={label}
  {disabled}
  onclick={toggle}
></button>

<style>
  .sw {
    position: relative;
    flex: none;
    width: 40px;
    height: 20px;
    padding: 0;
    border: 1px solid var(--text-2);
    border-radius: 10px;
    background: transparent;
    cursor: pointer;
    transition: background var(--duration-fast);
  }

  .sw:hover {
    background: var(--hover);
  }

  .sw::after {
    content: '';
    position: absolute;
    top: 3px;
    left: 4px;
    width: 12px;
    height: 12px;
    border-radius: 6px;
    background: var(--text-2);
    transition: left var(--duration) var(--ease);
  }

  .sw.on {
    border-color: transparent;
    background: var(--accent);
  }

  .sw.on:hover {
    background: var(--accent-hover);
  }

  .sw.on::after {
    left: 22px;
    background: var(--on-accent);
  }

  .sw:disabled {
    opacity: 0.45;
    pointer-events: none;
  }
</style>
