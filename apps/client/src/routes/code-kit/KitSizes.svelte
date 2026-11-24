<script lang="ts">
  const SIZES: readonly { readonly token: string; readonly t3: string; readonly axis: 'block' | 'inline' }[] = [
    { token: '--code-band', t3: 'Top band, 52 px', axis: 'block' },
    { token: '--code-card', t3: 'Card row, 78 px', axis: 'block' },
    { token: '--code-slim', t3: 'One-liner, 36 px', axis: 'block' },
    { token: '--code-shelf', t3: 'Shelf header, 32 px', axis: 'block' },
    { token: '--code-control', t3: 'Control, 32 px', axis: 'block' },
    { token: '--code-control-dense', t3: 'Dense control and menu item, 28 px', axis: 'block' },
    { token: '--code-kbd', t3: 'Kbd, 20 px', axis: 'block' },
    { token: '--code-icon', t3: 'Icon, 16 px', axis: 'block' },
    { token: '--code-inbox-min', t3: 'Inbox, at least 256 px', axis: 'inline' },
    { token: '--code-inbox-max', t3: 'Inbox, at most 320 px', axis: 'inline' },
    { token: '--code-lane', t3: 'Conversation lane, 46rem', axis: 'inline' },
  ];
</script>

<ul class="sizes">
  {#each SIZES as size (size.token)}
    <li>
      <code>{size.token}</code>
      <span class="t3">{size.t3}</span>
      <span class="bar {size.axis}" style:--size="var({size.token})"></span>
    </li>
  {/each}
</ul>

<style>
  .sizes {
    display: grid;
    gap: var(--space-3);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 12rem 16rem 1fr;
    align-items: center;
    gap: var(--space-4);
    font-size: var(--text-sm);
  }

  code {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
  }

  .t3 {
    color: var(--text-secondary);
  }

  .bar {
    border-radius: var(--radius-sm);
    background: var(--accent-tint);
    box-shadow: inset 0 0 0 var(--border-thin) var(--accent);
  }

  .block {
    inline-size: var(--space-16);
    block-size: var(--size);
  }

  .inline {
    inline-size: min(100%, var(--size));
    block-size: var(--space-3);
  }

  @media (width <= 60rem) {
    li {
      grid-template-columns: 1fr;
      gap: var(--space-1);
    }
  }
</style>
