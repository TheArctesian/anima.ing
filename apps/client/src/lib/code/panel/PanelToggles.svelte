<script lang="ts">
  import IconButton from '../kit/IconButton.svelte';
  import Tooltip from '../kit/Tooltip.svelte';

  /** Which parts show now. */
  export interface PanelsOpen {
    readonly details: boolean;
    readonly terminal: boolean;
    readonly panel: boolean;
  }

  interface Props {
    open: PanelsOpen;
    /** The right panel fills the row. */
    maximised?: boolean;
    /** No project: the terminal and the right panel have nothing to show, and their toggles are off. */
    project?: boolean;
    /** The words of each key as the system of the person shows them: "⇧⌘B", "⌘J", "⌥⌘B". */
    keys?: Partial<Record<keyof PanelsOpen, string>>;
    ontoggle: (part: keyof PanelsOpen) => void;
    onmaximise?: (() => void) | undefined;
  }

  const { open, maximised = false, project = true, keys = {}, ontoggle, onmaximise }: Props = $props();
  const named = (words: string, key: string | undefined): string => (key === undefined ? words : `${words} (${key})`);
</script>

<!--
  The toggles of the panels at the end of the top band (T3 Code `PanelLayoutControls.tsx`): thread details, the
  terminal drawer, the right panel, and "Maximize" while the right panel is open. They stand in the band, so a long
  title never runs under them.
-->
<div class="toggles" role="group" aria-label="Panels">
  <Tooltip text={named('Toggle thread details', keys.details)} side="below">
    <IconButton label="Toggle thread details" icon="square-menu" pressed={open.details} onclick={() => ontoggle('details')} />
  </Tooltip>
  <Tooltip text={named('Toggle terminal drawer', keys.terminal)} side="below">
    <IconButton label="Toggle terminal drawer" icon="panel-bottom" pressed={open.terminal} disabled={!project} onclick={() => ontoggle('terminal')} />
  </Tooltip>
  <Tooltip text={named('Toggle right panel', keys.panel)} side="below">
    <IconButton label="Toggle right panel" icon="panel-right" pressed={open.panel} disabled={!project} onclick={() => ontoggle('panel')} />
  </Tooltip>
  {#if open.panel && onmaximise !== undefined}
    <Tooltip text={maximised ? 'Restore right panel' : 'Maximize right panel'} side="below">
      <IconButton label={maximised ? 'Restore right panel' : 'Maximize right panel'} icon={maximised ? 'minimize-2' : 'maximize-2'} onclick={onmaximise} />
    </Tooltip>
  {/if}
</div>

<style>
  .toggles {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }
</style>
