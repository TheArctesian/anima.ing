<script lang="ts">
  import NotYetListed from '$lib/hosting/NotYetListed.svelte';
</script>

<!-- Applications (HSE-002): the one list of applications and containers belongs to the hosted applications plan (H42). -->
<NotYetListed
  title="Applications are not listed here yet"
  line="Hosting will list the applications and containers of each machine here, in one list. Until then, a machine's containers are on the Containers tab of its device page."
  link={{ href: '/devices', words: 'Open Devices' }}
/>
