<script lang="ts">
  import { untrack } from 'svelte';
  import { page } from '$app/state';
  import { devicesApi } from '$lib/devices/client';
  import { useHub } from '$lib/hub/context';
  import { mobileCodeApi, remoteCodeApi } from './api';
  import ThreadConversation from './ThreadConversation.svelte';
  import ThreadInbox from './ThreadInbox.svelte';
  import LaunchThread from './remote/LaunchThread.svelte';
  import { onReturn } from './live.svelte';
  import { recoveryScope, type RecoveryScope } from './remote/recovery';

  const { onnodes }: { onnodes?: () => void } = $props();
  const hub = useHub();
  const api = mobileCodeApi(hub);
  const remote = remoteCodeApi(hub);
  const connection = $derived(hub.status.current.connection);
  let starting = $state(false);
  let scope = $state.raw<RecoveryScope | null>(null);
  const startLabel = $derived(!starting && scope?.hasLaunch() ? 'Check previous start' : 'New thread');
  const id = $derived(page.url.searchParams.get('thread'));
  let machines = $state.raw<ReadonlyMap<string, string>>(new Map());

  $effect(() => {
    let active = true;
    void devicesApi(hub).deviceHosts().then((result) => {
      if (active && result.ok) machines = new Map(result.value.map((device) => [device.id, device.name]));
    });
    return () => { active = false; };
  });

  $effect(() => {
    let active = true;
    let checking = false;
    async function refreshOperator(): Promise<void> {
      if (checking) return;
      checking = true;
      const result = await remote.operator();
      checking = false;
      if (!active) return;
      const account = result.ok ? result.value?.id : undefined;
      if (scope?.active === true && account === scope.account) return;
      if (!result.ok && result.error.code !== 'UNAUTHENTICATED' && result.error.code !== 'FORBIDDEN') return;
      scope?.close();
      scope = account === undefined ? null : recoveryScope(hub.address.http, account, remote);
    }
    untrack(() => { void refreshOperator(); });
    const stop = onReturn(() => { void refreshOperator(); });
    return () => {
      active = false;
      stop();
      scope?.close();
    };
  });
</script>

<div class="mobile-code">
  {#if id === null}
    {#if starting && scope !== null}
      {#key scope}
        <LaunchThread api={remote} {scope} {machines} {connection} close={() => { starting = false; }} />
      {/key}
    {:else}
      <ThreadInbox {api} {machines} {onnodes} {startLabel} start={scope === null ? undefined : () => { starting = true; }} />
    {/if}
  {:else}
    {#key id}
      <ThreadConversation {api} {remote} {scope} {id} {machines} {connection} />
    {/key}
  {/if}
</div>

<style>
  .mobile-code {
    inline-size: 100%;
    max-inline-size: 48rem;
    min-inline-size: 0;
    margin-inline: auto;
  }
</style>
