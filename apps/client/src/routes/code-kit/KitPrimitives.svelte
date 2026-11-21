<script lang="ts">
  import CodeIcon from '$lib/code/icons/CodeIcon.svelte';
  import FormButton from '$lib/forms/FormButton.svelte';
  import IconButton from '$lib/code/kit/IconButton.svelte';
  import Kbd from '$lib/code/kit/Kbd.svelte';
  import Menu from '$lib/code/kit/Menu.svelte';
  import type { MenuEntry } from '$lib/code/kit/menu';
  import ScrollArea from '$lib/code/kit/ScrollArea.svelte';
  import Tooltip from '$lib/code/kit/Tooltip.svelte';
  import StatusMark from '$lib/code/status/StatusMark.svelte';
  import { STATUS_LOOKS, shortAge, type ChatStatus } from '$lib/code/status/status';

  const STATUSES = Object.keys(STATUS_LOOKS) as ChatStatus[];
  const NOW = Date.UTC(2026, 10, 21, 12);
  const AGES = [30_000, 5 * 60_000, 3 * 3_600_000, 2 * 86_400_000].map((age) => shortAge(NOW - age, NOW));

  const ROW_MENU: readonly MenuEntry[] = [
    { kind: 'choice', id: 'pin', label: 'Pin', icon: 'pin' },
    { kind: 'choice', id: 'rename', label: 'Rename', icon: 'square-pen', shortcut: 'F2' },
    { kind: 'choice', id: 'copy', label: 'Copy link', icon: 'copy', disabled: true },
    { kind: 'line' },
    { kind: 'group', label: 'Snooze until' },
    { kind: 'choice', id: 'hour', label: 'In an hour', detail: '13:00' },
    { kind: 'choice', id: 'tomorrow', label: 'Tomorrow', detail: 'Sat 09:00' },
    { kind: 'line' },
    { kind: 'choice', id: 'archive', label: 'Archive', icon: 'trash-2', danger: true },
  ];

  let chosen = $state('none');
  const lines = Array.from({ length: 24 }, (_, index) => `Chat ${String(index + 1)}`);
</script>

<div class="grid">
  <div class="cell" data-kit="status">
    <h3>Status (D11)</h3>
    <ul class="marks">
      {#each STATUSES as status (status)}<li><StatusMark {status} /></li>{/each}
    </ul>
    <p class="ages">No status: {AGES.join(', ')}</p>
  </div>

  <div class="cell" data-kit="kbd">
    <h3>Kbd</h3>
    <p class="row"><Kbd keys="⌘K" /> <Kbd keys="⇧⌘O" /> <Kbd keys="Esc" /></p>
    <p class="row"><Kbd keys="⌘K" outline /> <Kbd keys="Enter" outline /></p>
  </div>

  <div class="cell" data-kit="tooltip">
    <h3>Tooltip</h3>
    <p class="row">
      <Tooltip text="Settle thread"><FormButton size="sm"><CodeIcon name="check" />Settle</FormButton></Tooltip>
      <Tooltip text="Snooze thread"><IconButton label="Snooze" icon="clock" onclick={() => (chosen = 'snooze')} /></Tooltip>
    </p>
  </div>

  <div class="cell" data-kit="menu">
    <h3>Menu</h3>
    <p class="row">
      <Menu label="Chat actions" face={{ icon: 'ellipsis' }} entries={ROW_MENU} onChoose={(id) => (chosen = id)} />
      <Menu label="More options" face={{ text: 'More' }} entries={ROW_MENU} side={{ side: 'above', align: 'end' }} onChoose={(id) => (chosen = id)} />
      <Menu label="Locked options" face={{ icon: 'ellipsis' }} entries={ROW_MENU} disabled onChoose={(id) => (chosen = id)} />
    </p>
    <p class="ages" aria-live="polite">Chosen: {chosen}</p>
  </div>

  <div class="cell" data-kit="scroll">
    <h3>Scroll area</h3>
    <div class="row scrolls">
      <div class="box"><ScrollArea label="Chats with a bar">{#each lines as line (line)}<p class="line">{line}</p>{/each}</ScrollArea></div>
      <div class="box"><ScrollArea label="Chats with no bar" bar="hidden">{#each lines as line (line)}<p class="line">{line}</p>{/each}</ScrollArea></div>
    </div>
  </div>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
    gap: var(--space-6);
  }

  .cell {
    display: grid;
    align-content: start;
    gap: var(--space-3);
    padding: var(--space-4);
    border-radius: var(--radius-lg);
    background: var(--surface-raised);
  }

  h3 {
    margin: 0;
    font-size: var(--code-text-small);
    font-weight: var(--weight-medium);
  }

  .marks {
    display: grid;
    grid-template-columns: repeat(2, max-content);
    gap: var(--space-2) var(--space-6);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .ages {
    margin: 0;
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--space-3);
    margin: 0;
  }

  .scrolls {
    align-items: stretch;
  }

  .box {
    display: grid;
    flex: 1;
    block-size: 10rem;
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
  }

  .line {
    margin: 0;
    padding: var(--space-1) var(--space-3);
    font-size: var(--code-text-small);
  }
</style>
