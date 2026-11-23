<script lang="ts">
  import type { Snippet } from 'svelte';
  import { page } from '$app/state';
  import { provideHead } from '$lib/hosting/head.svelte';
  import HostingSections from '$lib/hosting/HostingSections.svelte';
  import { sectionOf } from '$lib/hosting/sections';
  import { resolve as entrance } from '$lib/motion/roles/resolve';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import SettingsSection from '$lib/settings/SettingsSection.svelte';

  const { children }: { children: Snippet } = $props();
  const current = $derived(sectionOf(page.url.pathname));
  const head = provideHead();
</script>

<!-- Hosting (HSE-001): what runs on the machines and how it is reached, with a list of sections on the left as Settings
     has. The frame is this page's; each section's body belongs to its owner. -->
<div class="head" {@attach entrance()}>
  <PageHeader title="Hosting" eyebrow={{ text: 'What runs on your machines', icon: 'carbon:data-base' }} actions={head.actions} />
</div>

<div class="hosting">
  <HostingSections {current} />
  <div class="page">
    {#if current.bare === true}
      <section aria-label={current.title}>{@render children()}</section>
    {:else}
      <SettingsSection id="hosting-{current.title.toLowerCase()}" title={current.title} icon={current.icon} description={current.description}>
        {@render children()}
      </SettingsSection>
    {/if}
  </div>
</div>

<style>
  .head {
    margin-block-end: var(--space-8);
  }

  /* The columns of Settings, so the two pages line up. */
  .hosting {
    display: grid;
    grid-template-columns: calc(var(--space-16) * 3 + var(--space-4)) minmax(0, calc(var(--space-16) * 11));
    gap: var(--space-12);
    align-items: start;
  }

  .page {
    min-inline-size: 0;
  }

  @media (max-width: 1100.98px) {
    .hosting {
      grid-template-columns: minmax(0, 1fr);
      gap: var(--space-5);
    }
  }
</style>
