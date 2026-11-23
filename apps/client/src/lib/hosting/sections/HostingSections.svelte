<script lang="ts">
  import { resolve } from '$app/paths';
  import { HOSTING_SECTIONS, type HostingSection } from './sections';

  interface Props {
    /** The section that is open. */
    current: HostingSection;
  }

  const { current }: Props = $props();
</script>

<!-- The sections of Hosting (HSE-001): a list on the left, as Settings has, each a page of its own; the Catalogue is a
     page below Applications, so it stands in. -->
<nav aria-label="Hosting sections">
  {#each HOSTING_SECTIONS as section (section.path)}
    <a href={resolve(section.path)} class:below={section.below === true} aria-current={section.path === current.path ? 'page' : undefined}>{section.title}</a>
  {/each}
</nav>

<style>
  nav {
    position: sticky;
    inset-block-start: var(--space-6);
    display: flex;
    flex-direction: column;
    gap: var(--border-focus);
  }

  a {
    display: flex;
    align-items: center;
    block-size: var(--control-md);
    padding: 0 var(--space-3);
    border-radius: var(--radius-md);
    color: var(--text-tertiary);
    font-size: var(--text-sm);
    text-decoration: none;
    white-space: nowrap;
    transition: color var(--duration-fast) var(--ease-enter), background-color var(--duration-fast) var(--ease-enter);
  }

  .below {
    padding-inline-start: var(--space-6);
  }

  a:hover {
    color: var(--text);
  }

  a[aria-current='page'] {
    background: var(--surface-hover);
    color: var(--text);
  }

  a:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  @media (max-width: 1100.98px) {
    nav {
      position: static;
      flex-flow: row wrap;
    }

    .below {
      padding-inline-start: var(--space-3);
    }
  }
</style>
