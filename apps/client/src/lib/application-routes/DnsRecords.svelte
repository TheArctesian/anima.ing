<script lang="ts">
  import CopyText from '$lib/forms/copy/CopyText.svelte';
  import type { ApplicationRoute } from './api';
  import { dnsRecords } from './checks';

  /**
   * Frames 2c and 2d: the records that prove the name of a route that is not proven yet, each with Copy. The hub gives
   * the TXT record of a hosted hub; it does not give the address of the front door or the public address of a hub that
   * a person hosts, so those records say so in place of a value.
   */
  const { route }: { route: ApplicationRoute } = $props();
  const records = $derived(dnsRecords(route));
  const hosted = $derived(route.proofRecordName !== null);
</script>

<section class="card" aria-label={hosted ? 'DNS records' : 'DNS record'}>
  {#if hosted}
    <div>
      <b>Add these two records in the DNS of {route.name}</b>
      <p class="note">The hub routes the name only after it finds the TXT record. One failed read removes nothing.</p>
    </div>
  {:else}
    <div>
      <b>Point the name at this hub</b>
      <p class="note">This hub is the only hub at its address, so the record is the proof. No TXT token is needed.</p>
    </div>
  {/if}
  {#each records as record (record.type)}
    <div class="record">
      <CopyText label="{record.type} name" text={record.name} />
      {#if record.value !== null}
        <CopyText label={record.valueLabel} text={record.value} />
      {:else}
        <p class="missing">
          {record.type === 'A'
            ? 'Address: the public address of this hub. The hub does not give it to this screen yet.'
            : 'Target: the address of the front door of Anima.ing. The hub does not give it to this screen yet.'}
        </p>
      {/if}
    </div>
  {/each}
</section>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-4);
    border: var(--border-thin) solid var(--border-subtle);
    border-radius: var(--radius-md);
    background: var(--surface-raised);
  }

  .note,
  .missing {
    margin: var(--space-1) 0 0;
    color: var(--text-secondary);
    font-size: var(--text-sm);
  }

  .record {
    display: grid;
    gap: var(--space-2);
  }
</style>
