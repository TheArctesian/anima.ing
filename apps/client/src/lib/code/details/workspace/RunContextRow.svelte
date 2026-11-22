<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { MenuEntry } from '../../kit/menu';
  import MenuPanel from '../../kit/MenuPanel.svelte';
  import Control from '../row/Control.svelte';
  import { environmentLabel, workspaceIcon, workspaceLabel } from '../labels';
  import { MACHINE_ICONS, type RunContext, type WorkspaceMode } from '../model';

  interface Props {
    context: RunContext;
    onEnvironment?: ((id: string) => void) | undefined;
    onMode?: ((mode: WorkspaceMode) => void) | undefined;
    /** "Copy full path" of the right-click menu. */
    onCopyPath?: ((path: string) => void) | undefined;
  }

  const { context, onEnvironment, onMode, onCopyPath }: Props = $props();
  const id = $props.id();
  const machine = $derived(context.environments.find((environment) => environment.id === context.environment));
  const workspace = $derived(workspaceLabel(context));
  const label = $derived(machine === undefined ? workspace : environmentLabel(context));
  const name = $derived(`Run context: ${machine === undefined ? workspace : `${machine.label}, ${workspace}`}`);
  let open = $state<'menu' | 'copy' | undefined>();
  let trigger = $state<HTMLButtonElement>();
  let pointer = $state({ left: 0, top: 0 });

  const entries = $derived<readonly MenuEntry[]>([
    ...(context.environments.length === 0
      ? []
      : ([
          { kind: 'group', label: 'Run on' },
          ...context.environments.map((environment) => ({
            kind: 'choice' as const,
            id: `on:${environment.id}`,
            label: environment.label,
            icon: MACHINE_ICONS[environment.kind],
            ...(environment.id === context.environment ? { detail: 'Current' } : {}),
            disabled: onEnvironment === undefined,
          })),
          { kind: 'line' },
        ] satisfies MenuEntry[])),
    { kind: 'group', label: 'Workspace' },
    { kind: 'choice', id: 'mode:local', label: context.worktreePath === undefined ? 'Current checkout' : 'Current worktree', icon: context.worktreePath === undefined ? 'folder' : 'folder-git', ...(context.mode === 'local' ? { detail: 'Current' } : {}), disabled: onMode === undefined },
    { kind: 'choice', id: 'mode:worktree', label: 'New worktree', icon: 'folder-git-2', ...(context.mode === 'worktree' ? { detail: 'Current' } : {}), disabled: onMode === undefined },
  ]);

  function choose(choice: string): void {
    const [kind, value] = choice.split(/:(.*)/);
    if (kind === 'on' && value !== undefined) onEnvironment?.(value);
    if (kind === 'mode') onMode?.(value === 'worktree' ? 'worktree' : 'local');
  }

  function oncontextmenu(event: MouseEvent): void {
    if (context.path === undefined || onCopyPath === undefined) return;
    event.preventDefault();
    event.stopPropagation();
    pointer = { left: event.clientX, top: event.clientY };
    open = 'copy';
  }
</script>

{#snippet content()}
  <CodeIcon name={machine === undefined ? workspaceIcon(context) : MACHINE_ICONS[machine.kind]} />
  <span class="details-label" title={context.path}>{label}</span>
  {#if machine !== undefined}
    <span class="end"><CodeIcon name={workspaceIcon(context)} /></span>
  {/if}
{/snippet}

<!-- Where the thread runs: its machine and its folder in one row, with one menu (07 section 1.1). -->
<div class="run" role="presentation" {oncontextmenu}>
  {#if context.locked}
    <span class="locked" role="img" aria-label={name}>{@render content()}</span>
  {:else}
    <Control still aria-label={name} aria-haspopup="menu" aria-expanded={open === 'menu'} aria-controls="{id}-menu" bind:element={trigger} onclick={() => (open = open === 'menu' ? undefined : 'menu')}>
      {@render content()}
      <span class="chevron"><CodeIcon name="chevron-down" /></span>
    </Control>
  {/if}
</div>
{#if open === 'menu'}
  <MenuPanel
    id="{id}-menu"
    label="Run context"
    {entries}
    from={trigger}
    anchor={() => trigger?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }}
    onChoose={choose}
    onClose={(back) => {
      open = undefined;
      if (back) trigger?.focus();
    }}
  />
{:else if open === 'copy'}
  <MenuPanel
    id="{id}-copy"
    label="Workspace path"
    entries={[{ kind: 'choice', id: 'copy', label: 'Copy full path', icon: 'copy' }]}
    anchor={() => ({ ...pointer, width: 0, height: 0 })}
    onChoose={() => {
      if (context.path !== undefined) onCopyPath?.(context.path);
    }}
    onClose={() => (open = undefined)}
  />
{/if}

<style>
  .run {
    display: flex;
    inline-size: 100%;
  }

  .locked {
    display: inline-flex;
    align-items: center;
    gap: var(--details-row-gap);
    inline-size: 100%;
    min-inline-size: 0;
    block-size: var(--details-row);
    padding: 0 var(--details-row-pad);
    color: var(--text);
    font-size: var(--code-text-small);
    font-weight: var(--weight-medium);
  }

  .end,
  .chevron {
    display: inline-flex;
    flex: none;
    color: var(--code-quiet-text);
  }

  .end {
    margin-inline-start: auto;
  }

  .end + .chevron {
    margin-inline-start: 0;
  }

  .chevron {
    margin-inline-start: auto;
  }
</style>
