<script lang="ts">
  import NotYetListed from '$lib/hosting/NotYetListed.svelte';
</script>

<!-- Storage (HSE-015): the volumes of each place belong to the hosted applications plan. -->
<NotYetListed
  title="Volumes are not listed here yet"
  line="The volumes of each machine will be listed here, with the application that uses each one. Until then, a machine's volumes are on the Containers tab of its device page."
  link={{ href: '/devices', words: 'Open Devices' }}
/>
