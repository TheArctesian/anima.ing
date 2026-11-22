<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import type { CodeIconName } from '../../icons/icons';
  import type { ChipKind } from './chips';

  interface Props {
    label: string;
    icon: CodeIconName;
    kind: ChipKind;
    /** The full words for the tip: the whole path of a file. */
    detail?: string;
    /** `invalid`: what it names is gone. `unresolved`: what it names is not known yet. */
    state?: 'invalid' | 'unresolved';
    /** Selected as one unit, as the editor selects a chip. */
    selected?: boolean;
  }

  const { label, icon, kind, detail, state, selected = false }: Props = $props();
</script>

<!-- An inline chip (03 "The chosen item as an inline chip"), drawn with the classes of mentions.css as the editor draws it. -->
<span class="code-mention-chip" data-kind={kind} data-state={state} data-selected={selected ? 'true' : undefined} title={detail}>
  <CodeIcon name={icon} />
  <span class="code-mention-chip-label">{label}</span>
</span>
