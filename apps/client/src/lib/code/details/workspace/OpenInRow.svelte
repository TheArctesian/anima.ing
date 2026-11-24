<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { MenuEntry } from '../../kit/menu';
  import MenuPanel from '../../kit/MenuPanel.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';
  import Control from '../row/Control.svelte';
  import type { OpenIn } from '../model';
  import Split from '../row/Split.svelte';

  interface Props {
    openIn: OpenIn;
    /** The key of "open in the preferred editor", for the menu: "⌘O". */
    shortcut?: string | undefined;
    /** Opens the folder of the project in the preferred editor. */
    onOpen?: (() => void) | undefined;
    /** A choice in the menu sets the preferred editor; it does not open it. */
    onPrefer?: ((id: string) => void) | undefined;
  }

  const { openIn, shortcut, onOpen, onPrefer }: Props = $props();
  const id = $props.id();
  const editor = $derived(openIn.editors.find((entry) => entry.id === openIn.preferred));
  const label = $derived(editor === undefined ? 'Open in editor' : `Open in ${editor.label}`);
  let open = $state(false);
  let group = $state<HTMLElement>();
  let trigger = $state<HTMLButtonElement>();

  const entries = $derived<readonly MenuEntry[]>(
    openIn.blocked !== undefined
      ? [{ kind: 'choice', id: 'none', label: openIn.blocked, disabled: true }]
      : openIn.editors.length === 0
        ? [{ kind: 'choice', id: 'none', label: 'No installed editors found', disabled: true }]
        : openIn.editors.map((entry) => ({
            kind: 'choice' as const,
            id: entry.id,
            label: entry.label,
            ...(entry.id === openIn.preferred ? (shortcut === undefined ? { detail: 'Preferred' } : { shortcut }) : {}),
            disabled: onPrefer === undefined,
          })),
  );
</script>

<!-- "Open in {editor}" (07 section 1.2): the main half opens the folder, the second half picks the editor. -->
<Split label="Open in editor" {open} bind:element={group}>
  {#snippet primary()}
    <Tooltip text={label}>
      <Control part="primary" data-open-favorite-editor blocked={editor === undefined || openIn.blocked !== undefined || onOpen === undefined} onclick={() => onOpen?.()}>
        <CodeIcon name="square-arrow-out-up-right" />
        <span class="details-label">{label}</span>
      </Control>
    </Tooltip>
  {/snippet}
  {#snippet secondary()}
    <Tooltip text="Choose editor">
      <Control part="secondary" still aria-label="Choose editor" aria-haspopup="menu" aria-expanded={open} aria-controls="{id}-menu" bind:element={trigger} onclick={() => (open = !open)}>
        <CodeIcon name="chevron-down" />
      </Control>
    </Tooltip>
  {/snippet}
</Split>
{#if open}
  <MenuPanel
    id="{id}-menu"
    label="Choose editor"
    {entries}
    side={{ side: 'below', align: 'end' }}
    from={trigger}
    anchor={() => group?.getBoundingClientRect() ?? { left: 0, top: 0, width: 0, height: 0 }}
    onChoose={(choice) => onPrefer?.(choice)}
    onClose={(back) => {
      open = false;
      if (back) trigger?.focus();
    }}
  />
{/if}
