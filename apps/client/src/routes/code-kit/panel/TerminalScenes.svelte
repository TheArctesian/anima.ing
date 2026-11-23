<script lang="ts">
  import { NO_TERMINALS, activated, added, ended, type SplitDirection, type TerminalGroup, type TerminalPane } from '$lib/code/panel/terminal/group';
  import TerminalSurface from '$lib/code/panel/terminal/TerminalSurface.svelte';
  import { closeQuestion, failureWords } from '$lib/code/panel/terminal/words';

  interface Props {
    say: (what: string) => (...values: unknown[]) => void;
  }

  const { say }: Props = $props();
  const KEYS = { split_horizontal: '⌘D', split_vertical: '⇧⌘D', new: '⌘N', close: '⌘W' };
  const OUTPUT = '~/worktrees/fix-login $ bun run test\n 14 pass\n 0 fail\n~/worktrees/fix-login $ ';
  const of = (direction: SplitDirection, ...ids: string[]): TerminalGroup => ids.reduce((group, id) => added(group, { id, label: `zsh ${id}` }, direction), NO_TERMINALS);
  let live = $state<TerminalGroup>(of('horizontal', '1'));
  let next = 2;

  function split(direction: SplitDirection): void {
    live = added(live, { id: String(next), label: `zsh ${String(next)}` }, direction);
    next += 1;
  }

  function close(id: string): void {
    say('close')(closeQuestion(live.terminals.filter((each) => each.id === id)).title);
    live = ended(live, id);
  }
</script>

{#snippet pane(terminal: TerminalPane, active: boolean)}
  <pre class:active>{terminal.id === 'failed' ? failureWords() : `${OUTPUT}${active ? '█' : ''}`}</pre>
{/snippet}

<div class="scenes">
  <section data-kit="terminal-live" aria-label="Terminal, one that the buttons change">
    <TerminalSurface group={live} {pane} keys={KEYS} onsplit={split} onnew={say('new terminal tab')} onactivate={(id) => (live = activated(live, id))} onclose={close} />
  </section>
  <section data-kit="terminal-side" aria-label="Terminal, side by side">
    <TerminalSurface group={of('horizontal', '1', '2')} {pane} keys={KEYS} onsplit={say('split')} onnew={say('new')} onactivate={say('activate')} onclose={say('close')} />
  </section>
  <section data-kit="terminal-stacked" aria-label="Terminal, stacked, at the limit">
    <TerminalSurface group={of('vertical', '1', '2', '3', '4')} {pane} keys={KEYS} onsplit={say('split')} onnew={say('new')} onactivate={say('activate')} onclose={say('close')} />
  </section>
  <section data-kit="terminal-locked" aria-label="Terminal, no permission">
    <TerminalSurface group={of('horizontal', 'failed')} {pane} permitted={false} onsplit={say('split')} onnew={say('new')} onactivate={say('activate')} onclose={say('close')} />
  </section>
  <section data-kit="terminal-empty" aria-label="Terminal, none yet">
    <TerminalSurface group={NO_TERMINALS} {pane} keys={KEYS} onsplit={say('split')} onnew={say('new terminal')} onactivate={say('activate')} onclose={say('close')} />
  </section>
</div>

<style>
  .scenes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
  }

  section {
    display: flex;
    inline-size: calc(var(--space-16) * 8.5);
    block-size: calc(var(--space-16) * 5.5);
    overflow: hidden;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
  }

  pre {
    flex: 1;
    margin: 0;
    padding: var(--space-2);
    overflow: hidden;
    background: var(--surface-sunken);
    white-space: pre-wrap;
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--code-text-meta);
  }
</style>
