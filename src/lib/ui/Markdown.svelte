<script lang="ts">
  import { parseMarkdown, type Block, type Inline } from '../markdown';

  interface Props {
    source: string;
    /** Открыть ссылку (адрес проверяет Rust: только http и https). */
    onlink: (href: string) => void;
  }

  let { source, onlink }: Props = $props();

  const blocks = $derived(parseMarkdown(source));
</script>

{#snippet inline(nodes: Inline[])}
  {#each nodes as node, i (i)}
    {#if node.t === 'text'}{node.v}{:else if node.t === 'code'}<code>
        {node.v}
      </code>{:else if node.t === 'strong'}<strong>
        {@render inline(node.c)}
      </strong>{:else if node.t === 'em'}<em>
        {@render inline(node.c)}
      </em>{:else if node.t === 'del'}<del>
        {@render inline(node.c)}
      </del>{:else if node.t === 'link'}<button
        type="button"
        class="link"
        title={node.href}
        onclick={() => onlink(node.href)}
      >
        {@render inline(node.c)}
      </button>{:else if node.t === 'br'}<br />{/if}
  {/each}
{/snippet}

{#snippet blockList(list: Block[])}
  {#each list as block, i (i)}
    {#if block.t === 'heading'}
      <svelte:element this={`h${Math.min(block.level + 2, 6)}`} class="h h{block.level}">
        {@render inline(block.c)}
      </svelte:element>
    {:else if block.t === 'paragraph'}
      <p>{@render inline(block.c)}</p>
    {:else if block.t === 'list'}
      <svelte:element
        this={block.ordered ? 'ol' : 'ul'}
        start={block.ordered ? block.start : undefined}
      >
        {#each block.items as item, j (j)}
          <li class:task={item.task !== null}>
            {#if item.task !== null}
              <input
                type="checkbox"
                checked={item.task}
                disabled
                aria-label={item.task ? 'выполнено' : 'не выполнено'}
              />
              <span class:done={item.task}>{@render inline(item.c)}</span>
            {:else}
              {@render inline(item.c)}
            {/if}
            {#if item.children.length}{@render blockList(item.children)}{/if}
          </li>
        {/each}
      </svelte:element>
    {:else if block.t === 'code'}
      <pre><code>{block.v}</code></pre>
    {:else if block.t === 'quote'}
      <blockquote>{@render blockList(block.c)}</blockquote>
    {:else if block.t === 'hr'}
      <hr />
    {/if}
  {/each}
{/snippet}

<div class="md selectable">
  {@render blockList(blocks)}
</div>

<style>
  .md {
    overflow-wrap: anywhere;
  }

  .md > :global(:last-child) {
    margin-bottom: 0;
  }

  .md :global(.h) {
    margin: 0 0 6px;
    font-family: var(--font-ui);
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .md :global(.h1) {
    font-size: 17px;
  }

  .md :global(* + .h) {
    margin-top: 4px;
  }

  .md p,
  .md :global(ul),
  .md :global(ol),
  .md pre,
  .md blockquote {
    margin: 0 0 12px;
  }

  .md :global(ul),
  .md :global(ol) {
    padding-left: 20px;
  }

  .md :global(li) {
    margin: 2px 0;
  }

  .md :global(li > ul),
  .md :global(li > ol) {
    margin: 2px 0 0;
  }

  .md li.task {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 8px;
    margin-left: -20px;
    list-style: none;
  }

  .md li.task input {
    position: relative;
    top: 2px;
  }

  .md li.task > :global(ul),
  .md li.task > :global(ol) {
    flex-basis: 100%;
    padding-left: 44px;
  }

  .done {
    color: var(--text-2);
    text-decoration: line-through;
  }

  pre {
    overflow-x: auto;
    padding: 10px 12px;
    border-radius: var(--radius);
    background: var(--hover);
  }

  pre code {
    padding: 0;
    background: none;
  }

  blockquote {
    padding-left: 12px;
    border-left: 3px solid var(--border-strong);
    color: var(--text-2);
  }

  hr {
    margin: 0 0 12px;
    border: 0;
    border-top: 1px solid var(--border);
  }

  .link {
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .link:hover {
    text-decoration: underline;
  }
</style>
