<script lang="ts">
  import FormButton from '$lib/forms/FormButton.svelte';
  import Notice from '$lib/states/Notice.svelte';
  import ConfirmModal from '$lib/forms/modal/ConfirmModal.svelte';
  import type { Group } from './clusters';
  import { ProjectGroups } from './groups.svelte';
  import type { SharedProjectGroups } from './shared.svelte';

  const { groups }: { groups: SharedProjectGroups } = $props();
  const local = new ProjectGroups(localStorage);
  let sharing = $state<Group | null>(null);

  async function share(group: Group) {
    const problem = await groups.create(group.name, local.members(group.id));
    if (problem !== null) return { ok: false as const, error: { code: 'CONFLICT' as const, message: problem } };
    local.delete(group.id);
    return { ok: true as const, value: null };
  }
</script>

{#if local.groups.length > 0}
  <Notice tone="info" title="Groups on this device" line="You can share these groups with everyone on this hub." />
  <div class="actions">
    {#each local.groups as group (group.id)}
      <FormButton size="sm" onclick={() => (sharing = group)}>Share {group.name}</FormButton>
    {/each}
  </div>
{/if}
{#if sharing !== null}
  {@const group = sharing}
  <ConfirmModal
    open
    title={`Share ${group.name} with this hub?`}
    body="Everyone on this hub can see the group. Sharing moves these projects from any shared groups they already belong to."
    confirmLabel="Share group"
    onConfirm={() => share(group)}
    onCancel={() => (sharing = null)}
  />
{/if}

<style>
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
</style>
