<script lang="ts">
  import type { SVGAttributes } from 'svelte/elements';
  let props: SVGAttributes<SVGSVGElement> = $props();
</script>

<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" {...props}>
  <circle cx="12" cy="12" r="7.5" stroke="currentColor" stroke-width="1.7" />
  <path d="M3 22 17 7 9 13 21 2 7 17Z" fill="currentColor" />
</svg>
