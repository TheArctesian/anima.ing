<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';

  const id = $props.id();
</script>

<!-- Add a route, in the header of Hosting > Routes (mockup routes, Frame 1). A route goes to an application that the hub
     has recorded with its ports (task H6), so it is off with that reason until then. -->
<div class="add">
  <FormButton tone="primary" icon="carbon:add" aria-disabled="true" aria-describedby="{id}-why">Add a route</FormButton>
  <p class="why" id="{id}-why">Not built: a route goes to an application that the hub has recorded with its ports, and the hub keeps no such record yet (task H6).</p>
</div>

<style>
  .add {
    display: flex;
    flex-direction: column;
    align-items: end;
    gap: var(--space-1);
    max-inline-size: calc(var(--space-16) * 4);
  }

  .why {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    text-align: end;
  }
</style>
