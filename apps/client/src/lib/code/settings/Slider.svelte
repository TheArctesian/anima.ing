<script lang="ts">
  interface Props {
    id: string;
    value: number;
    min: number;
    max: number;
    step: number;
    /** The value as the chip beside the slider writes it, for example "120%". */
    shown: string;
    onchange: (value: number) => void;
  }

  const { id, value, min, max, step, shown, onchange }: Props = $props();
</script>

<!-- T3 Code's settings slider: a value chip, then the range (208 px), in Anima's skin. -->
<span class="slider">
  <output class="chip" for={id}>{shown}</output>
  <input
    {id}
    type="range"
    {min}
    {max}
    {step}
    {value}
    aria-valuetext={shown}
    oninput={(event) => {
      onchange(Number(event.currentTarget.value));
    }}
  />
</span>

<style>
  .slider {
    display: inline-flex;
    min-inline-size: 0;
    max-inline-size: 100%;
    align-items: center;
    gap: var(--space-3);
  }

  .chip {
    flex: none;
    box-sizing: border-box;
    min-inline-size: var(--space-12);
    padding: var(--space-1) var(--space-2);
    border-radius: var(--radius-md);
    background-color: var(--surface-sunken);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
    font-weight: 500;
    text-align: center;
  }

  /* The row's shared input look gives a border and a fixed width; a range takes neither. */
  .slider input {
    min-inline-size: 0;
    inline-size: calc(var(--space-16) * 3.25);
    block-size: var(--target-min-touch);
    padding: 0;
    border: 0;
    background: transparent;
    accent-color: var(--accent);
  }
</style>
