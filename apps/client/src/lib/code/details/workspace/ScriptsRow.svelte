<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { MenuEntry } from '../../kit/menu';
  import MenuPanel from '../../kit/MenuPanel.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import { scriptLabel } from '../labels';
  import { SCRIPT_ICONS, type Scripts } from '../model';
  import Split from '../row/Split.svelte';

  interface Props {
    scripts: Scripts;
    onRun?: ((id: string) => void) | undefined;
    /** Opens the form of a new script. */
    onAdd?: (() => void) | undefined;
    /** Opens the form of one script. */
    onEdit?: ((id: string) => void) | undefined;
  }

  const { scripts, onRun, onAdd, onEdit }: Props = $props();
  const id = $props.id();
  const main = $derived(scripts.scripts.find((script) => script.id === scripts.primary));
  const cannotRun = $derived(scripts.blocked !== undefined || onRun === undefined);
  let open = $state(false);
  let group = $state<HTMLElement>();
  let trigger = $state<HTMLButtonElement>();

  const entries = $derived<readonly MenuEntry[]>([
    ...scripts.scripts.map((script) => ({
      kind: 'choice' as const,
      id: `run:${script.id}`,
      label: scriptLabel(script),
      icon: SCRIPT_ICONS[script.icon],
      ...(script.shortcut === undefined ? {} : { shortcut: script.shortcut }),
      disabled: cannotRun,
    })),
    ...(onEdit === undefined || scripts.scripts.length === 0
      ? []
      : ([{ kind: 'line' }, { kind: 'group', label: 'Edit' }, ...scripts.scripts.map((script) => ({ kind: 'choice' as const, id: `edit:${script.id}`, label: `Edit ${script.name}`, icon: 'settings' as const }))] satisfies MenuEntry[])),
    { kind: 'line' },
    { kind: 'choice', id: 'add', label: 'Add project script', icon: 'plus', disabled: onAdd === undefined },
  ]);

  function choose(choice: string): void {
    const [kind, value] = choice.split(/:(.*)/);
    if (kind === 'add') onAdd?.();
    if (kind === 'run' && value !== undefined) onRun?.(value);
    if (kind === 'edit' && value !== undefined) onEdit?.(value);
  }
</script>

<!--
  The scripts of the project (07 section 1.3) in its three shapes: a script to run with a menu, "Actions" with a menu,
  or "Add project script". T3 Code puts the edit button inside the row of the menu; our menu has a group "Edit".
-->
{#if scripts.scripts.length === 0}
  <Tooltip text="Add project script">
    <Control blocked={onAdd === undefined} onclick={() => onAdd?.()}>
      <CodeIcon name="plus" />
      <span class="details-label">Add project script</span>
    </Control>
  </Tooltip>
{:else}
  <Split label={main === undefined ? 'Project actions' : 'Project scripts'} {open} bind:element={group}>
    {#snippet primary()}
      {#if main === undefined}
        <Control part="primary" still onclick={() => (open = !open)}>
          <CodeIcon name="wrench" />
          <span class="details-label">Actions</span>
        </Control>
      {:else}
        <Tooltip text={scripts.blocked ?? `Run ${main.name}`}>
          <Control part="primary" aria-label="Run {main.name}" blocked={cannotRun} onclick={() => onRun?.(main.id)}>
            <CodeIcon name={SCRIPT_ICONS[main.icon]} />
            <span class="details-label">{main.name}</span>
          </Control>
        </Tooltip>
      {/if}
    {/snippet}
    {#snippet secondary()}
      {@const name = main === undefined ? 'Choose project action' : 'Script actions'}
      <Tooltip text={name}>
        <Control part="secondary" still aria-label={name} aria-haspopup="menu" aria-expanded={open} aria-controls="{id}-menu" bind:element={trigger} onclick={() => (open = !open)}>
          <CodeIcon name="chevron-down" />
        </Control>
      </Tooltip>
    {/snippet}
  </Split>
{/if}
{#if open}
  <MenuPanel
    id="{id}-menu"
    label="Project scripts"
    {entries}
    side={{ side: 'below', align: 'end' }}
    from={group}
    anchor={() => group?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }}
    onChoose={choose}
    onClose={(back) => {
      open = false;
      if (back) trigger?.focus();
    }}
  />
{/if}
