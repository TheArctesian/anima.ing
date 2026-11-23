<script lang="ts">
  import '$lib/code/kit/code.css';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import Section from '$lib/page/Section.svelte';
  import KitApprovals from './KitApprovals.svelte';
  import KitBand from './KitBand.svelte';
  import KitIcons from './KitIcons.svelte';
  import KitPrimitives from './KitPrimitives.svelte';
  import KitSizes from './KitSizes.svelte';

  let width = $state<'comfortable' | 'wide' | 'full'>('comfortable');
  let text = $state<'normal' | 'large' | 'larger'>('normal');
  let contrast = $state<'normal' | 'more'>('normal');
</script>

<div class="kit" data-code-width={width} data-code-text={text} data-code-contrast={contrast}>
  <PageHeader title="Code kit" detail="T3 Code's layout in Anima's skin" eyebrow={{ text: 'Stage 1', icon: 'carbon:code' }} />
  <div class="settings">
    <Segmented label="Chat width" size="sm" bind:value={width} options={[{ value: 'comfortable', label: 'Comfortable' }, { value: 'wide', label: 'Wide' }, { value: 'full', label: 'Full' }]} />
    <Segmented label="Text size" size="sm" bind:value={text} options={[{ value: 'normal', label: 'Normal' }, { value: 'large', label: 'Large' }, { value: 'larger', label: 'Larger' }]} />
    <Segmented label="Contrast" size="sm" bind:value={contrast} options={[{ value: 'normal', label: 'Normal' }, { value: 'more', label: 'More' }]} />
  </div>
  <Section title="Sizes" icon="carbon:corner"><KitSizes /></Section>
  <Section title="Top band" icon="carbon:side-panel-close"><KitBand /></Section>
  <Section title="Status, kbd, tooltip, menu, scroll area" icon="carbon:cursor-1"><KitPrimitives /></Section>
  <Section title="Approvals" icon="carbon:security"><KitApprovals /></Section>
  <Section title="Lucide icons of the Code section" icon="carbon:catalog"><KitIcons /></Section>
</div>

<style>
  .kit {
    display: grid;
    gap: var(--space-10);
    max-inline-size: var(--content-page);
    padding-block-end: var(--space-16);
  }

  .settings {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }
</style>
