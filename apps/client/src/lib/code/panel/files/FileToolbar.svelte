<script lang="ts">
  import type { Snippet } from 'svelte';
  import type { CodeIconName } from '../../icons/icons';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';

  interface Props {
    /** "Open in editor": the host draws its own control here, when the environment can open editors. */
    openIn?: Snippet | undefined;
    /** The rendered / source toggle: its words and icon for what shows now; left out for a kind with one view. */
    toggle?: { readonly label: string; readonly icon: CodeIconName } | undefined;
    ontoggle?: () => void;
    /** Word wrap is on; `undefined` while no raw text shows, and the control is left out. */
    wrap?: boolean | undefined;
    onwrap?: () => void;
    /** Copies the contents of the file; left out for a file that is cut short, empty or not text. */
    oncopy?: (() => Promise<void>) | undefined;
    /** Opens the file in the preview browser; left out when it cannot. */
    onpreview?: (() => void) | undefined;
    /** The file explorer shows; `undefined` for a file that has none. */
    explorer?: boolean | undefined;
    onexplorer?: () => void;
  }

  const { openIn, toggle, ontoggle, wrap, onwrap, oncopy, onpreview, explorer, onexplorer }: Props = $props();
  let copy = $state<'idle' | 'copied' | 'failed'>('idle');
  const copyWords = $derived(copy === 'copied' ? 'Copied' : copy === 'failed' ? 'Could not copy file' : 'Copy contents');
  const wrapWords = $derived(wrap === true ? 'Disable word wrap' : 'Enable word wrap');
  const explorerWords = $derived(explorer === true ? 'Hide file explorer' : 'Show file explorer');

  async function copyFile(): Promise<void> {
    try {
      await oncopy?.();
      copy = 'copied';
    } catch {
      // No clipboard, or the system refused it; the button says so and the text stays selectable.
      copy = 'failed';
    }
    setTimeout(() => (copy = 'idle'), 2000);
  }
</script>

<!--
  The toolbar of a file (report 05, Section 3.4), in T3 Code's order. A control whose words say what a press does
  ("Enable word wrap") has no pressed state: the words are its state. There is no save button: a file is read-only here.
-->
<div class="toolbar" role="toolbar" aria-label="File actions">
  {#if openIn !== undefined}{@render openIn()}{/if}
  {#if toggle !== undefined}
    <Tooltip text={toggle.label} side="below"><IconButton label={toggle.label} icon={toggle.icon} onclick={() => ontoggle?.()} /></Tooltip>
  {/if}
  {#if wrap !== undefined}
    <Tooltip text={wrapWords} side="below"><IconButton label={wrapWords} icon="wrap-text" onclick={() => onwrap?.()} /></Tooltip>
  {/if}
  {#if oncopy !== undefined}
    <Tooltip text={copyWords} side="below">
      <IconButton label={copyWords} icon={copy === 'copied' ? 'check' : 'copy'} onclick={() => void copyFile()} />
    </Tooltip>
    <span class="said" role="status">{copy === 'idle' ? '' : copyWords}</span>
  {/if}
  {#if onpreview !== undefined}
    <Tooltip text="Open file in preview browser" side="below">
      <IconButton label="Open file in preview browser" icon="globe" onclick={onpreview} />
    </Tooltip>
  {/if}
  {#if explorer !== undefined}
    <Tooltip text={explorerWords} side="below"><IconButton label={explorerWords} icon="folder-tree" onclick={() => onexplorer?.()} /></Tooltip>
  {/if}
</div>

<style>
  .toolbar {
    display: flex;
    flex: none;
    align-items: center;
    gap: var(--space-1);
  }

  .said {
    position: absolute;
    inline-size: 1px;
    block-size: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
