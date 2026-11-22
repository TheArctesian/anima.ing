<script lang="ts">
  import { CODE_ICONS, type CodeIconName } from './icons';

  interface Props {
    name: CodeIconName;
    /** 16, 18 or 20 px, the icon tokens of the app. */
    size?: 'sm' | 'md' | 'lg';
  }

  const { name, size = 'sm' }: Props = $props();
  const Shape = $derived(CODE_ICONS[name]);
</script>

<!-- A Lucide icon at the app's stroke of 1.5 (D4), drawn only; the control around it carries the name. -->
<Shape class="code-icon code-icon-{size}" strokeWidth={1.5} aria-hidden="true" focusable="false" />

<style>
  :global(.code-icon) {
    display: inline-block;
    flex: none;
    vertical-align: -0.125em;
  }

  :global(.code-icon-sm) {
    inline-size: var(--icon-sm);
    block-size: var(--icon-sm);
  }

  :global(.code-icon-md) {
    inline-size: var(--icon-md);
    block-size: var(--icon-md);
  }

  :global(.code-icon-lg) {
    inline-size: var(--icon-lg);
    block-size: var(--icon-lg);
  }
</style>
