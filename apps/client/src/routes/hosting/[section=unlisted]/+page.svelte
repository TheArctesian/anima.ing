<script lang="ts">
  import { page } from '$app/state';
  import NotYetListed from '$lib/hosting/sections/NotYetListed.svelte';
  import { UNLISTED, type Unlisted } from '$lib/hosting/sections/unlisted';

  // The matcher `unlisted` lets only the names of UNLISTED reach this page.
  const words = $derived(UNLISTED[page.params['section'] as Unlisted]);
</script>

<!-- Models of Hosting: each says that its list is not built yet, and where the thing is today. -->
<NotYetListed title={words.title} line={words.line} link={words.devices ? { href: '/devices', words: 'Open Devices' } : undefined} />
