<script lang="ts">
  import type { Tone } from './route';

  const { tone, label }: { tone: Tone; label: string } = $props();
</script>

<!-- The colour is on the dot; the word stays the text colour, so it never relies on colour alone (UI-003, MAT-005). -->
<span class="word {tone}">{label}</span>

<style>
  .word {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
  }

  .word::before {
    content: '';
    flex: none;
    inline-size: var(--space-2);
    block-size: var(--space-2);
    border-radius: var(--radius-pill);
    background: var(--tone);
  }

  .up {
    --tone: var(--status-up);
  }

  .degraded {
    --tone: var(--status-degraded);
  }

  .down {
    --tone: var(--status-down);
  }

  .unknown {
    --tone: var(--status-unknown);
  }
</style>
