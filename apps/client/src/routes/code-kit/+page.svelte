<script lang="ts">
  import '$lib/code/kit/code.css';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import Section from '$lib/page/Section.svelte';
  import KitApprovals from './KitApprovals.svelte';
  import KitIcons from './KitIcons.svelte';
  import KitPrimitives from './KitPrimitives.svelte';
  import KitSizes from './KitSizes.svelte';
</script>

<!-- Stage 1 of the T3 Code copy: each size and primitive of the Code section, in each state, for the gallery. -->
<div class="kit">
  <PageHeader title="Code kit" detail="T3 Code's layout in Anima's skin" eyebrow={{ text: 'Stage 1', icon: 'carbon:code' }} />
  <Section title="Sizes" icon="carbon:corner"><KitSizes /></Section>
  <Section title="Status, kbd, tooltip, menu, scroll area" icon="carbon:cursor-1"><KitPrimitives /></Section>
  <Section title="Approvals" icon="carbon:security"><KitApprovals /></Section>
  <Section title="Icons" icon="carbon:carbon"><KitIcons /></Section>
</div>

<style>
  .kit {
    display: grid;
    gap: var(--space-10);
    max-inline-size: var(--content-page);
    padding-block-end: var(--space-16);
  }
</style>
