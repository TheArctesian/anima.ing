<script lang="ts">
  import { onMount } from 'svelte';
  import { linkPorts } from '$lib/desktop/link/shell';
  import Switch from '$lib/forms/switch/Switch.svelte';
  import SettingRow from '$lib/settings/SettingRow.svelte';
  import { portRows, systemOf, type PortRow } from './ports';

  // HMD-048, HMD-049: saved choices are never reset by the setup defaults; the hub cannot change them.
  const system = systemOf(navigator.userAgent);
  let rows = $state<readonly PortRow[] | null>(null);
  let busy = $state(false);
  let problem = $state<string | null>(null);
  /** Goes up when a change failed: the switches are drawn again from the list, since each keeps its own state. */
  let drawn = $state(0);

  onMount(() => {
    void linkPorts().then((result) => {
      if (result.ok) rows = portRows(system, result.value.open);
    });
  });

  async function turn(port: number, on: boolean): Promise<void> {
    if (busy) return;
    busy = true;
    const result = await linkPorts({ port, on });
    busy = false;
    if (result.ok) {
      rows = portRows(system, result.value.open);
      problem = null;
    } else {
      drawn += 1;
      problem = `Could not turn the port ${on ? 'on' : 'off'}. ${result.message}`;
    }
  }
</script>

{#if rows !== null}
  {#each rows as row (`${String(row.port)}-${String(drawn)}`)}
    <SettingRow label={row.label} for="open-port-{row.port}">
      {#snippet hint()}<p class="hint" id="open-port-{row.port}-hint">{row.needs}</p>{/snippet}
      <Switch
        id="open-port-{row.port}"
        checked={row.on}
        describedby="open-port-{row.port}-hint"
        disabled={busy}
        onchange={(on) => {
          void turn(row.port, on);
        }}
      />
    </SettingRow>
  {/each}
  {#if busy}<p role="status">Preparing private access… Check for a system authorization prompt.</p>{/if}
  <p class="problem" role="alert">{problem ?? ''}</p>
{/if}

<style>
  .hint {
    margin: 0;
  }

  .problem {
    margin: 0;
    color: var(--danger);
    font-size: var(--text-sm);
  }

  .problem:empty {
    display: none;
  }
</style>
