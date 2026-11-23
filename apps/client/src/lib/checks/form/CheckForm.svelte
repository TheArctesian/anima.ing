<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { SvelteSet } from 'svelte/reactivity';
  import { fieldErrorsFrom } from '$lib/devices/form/field-errors';
  import Disclosure from '$lib/forms/disclosure/Disclosure.svelte';
  import { modalFooter } from '$lib/forms/modal/footer';
  import type { FieldError, HubResult } from '$lib/hub/hub';
  import AdvancedFields from './fields/AdvancedFields.svelte';
  import GroupFields from './fields/GroupFields.svelte';
  import HttpFields from './fields/HttpFields.svelte';
  import PressButton from './fields/PressButton.svelte';
  import SelectField from './fields/SelectField.svelte';
  import TextField from './fields/TextField.svelte';
  import TypeFields from './fields/TypeFields.svelte';
  import ProbeFields from './probes/ProbeFields.svelte';
  import DomainExpiryFields from './probes/DomainExpiryFields.svelte';
  import { contextHubIsHosted } from '$lib/hosted/context';
  import { contextChannelCalls } from './channels/calls';
  import { ChannelsDraft } from './channels/draft.svelte';
  import ChannelsSection from './channels/ChannelsSection.svelte';
  import { intervalFor } from './probes/domain-expiry';
  import SnmpFields from './probes/SnmpFields.svelte';
  import { defaultsReader } from '../defaults/reader';
  import { untouchedSettings, type SettingKey } from '../defaults/settings';
  import { messageFor, placeHubErrors, shownFields, touchesAdvanced, unplaced } from './model/errors';
  import { groupCandidates, type SiblingCheck } from './model/group';
  import type { CheckFields, CheckType } from './model/hub-types';
  import { validateCheck } from './model/validate';
  import { secretWarnings } from './model/warnings';
  import { checkValues, type CheckFormValues } from './model/values';

  interface Props {
    check: CheckFields | null;
    siblings: readonly SiblingCheck[];
    onSave: (values: CheckFormValues) => Promise<HubResult<unknown>>;
    /** The owner closes the form here, not on `onSave`, which would hide a failed channel save. */
    onDone?: () => void;
    onCancel: () => void;
  }

  const { check, siblings, onSave, onDone, onCancel }: Props = $props();

  const ALL_TYPES: readonly { value: CheckType; label: string }[] = [
    { value: 'ICMP', label: 'Ping (ICMP)' },
    { value: 'TCP', label: 'TCP port' },
    { value: 'TLS', label: 'TLS certificate' },
    { value: 'HTTP', label: 'HTTP or HTTPS' },
    { value: 'DNS', label: 'DNS lookup' },
    { value: 'PUSH', label: 'Push (device calls the hub)' },
    { value: 'GROUP', label: 'Group of checks' },
    { value: 'DOCKER', label: 'Docker container' },
    { value: 'SYSTEMD', label: 'systemd service' },
    { value: 'SNMP', label: 'SNMP' },
    { value: 'TAILSCALE_PING', label: 'Tailscale ping' },
    { value: 'DOMAIN_EXPIRY', label: 'Domain expiry' },
  ];
  // A hosted hub has no VPN, so it offers no Tailscale ping; a check that already is one keeps its type (R-222).
  const hosted = contextHubIsHosted();
  const TYPES = ALL_TYPES.filter((entry) => !hosted || entry.value !== 'TAILSCALE_PING' || untrack(() => check?.type) === 'TAILSCALE_PING');

  const footer = modalFooter();
  const formId = $props.id();
  $effect.pre(() => footer?.show(actions));

  const initial = untrack(() => checkValues(check, 'ICMP'));
  let values = $state(initial);
  // D-26: the hub's defaults never hold the form, and a setting the operator has touched stays.
  const touched = new SvelteSet<SettingKey>();
  let ordinaryInterval = untrack(() => initial.intervalSeconds);
  const readDefaults = untrack(() => (check === null ? defaultsReader() : null));
  $effect(() => {
    void readDefaults?.().then((defaults) => {
      if (defaults === null) return;
      const patch = untouchedSettings(touched, defaults);
      if (patch.intervalSeconds !== undefined) {
        ordinaryInterval = patch.intervalSeconds;
        patch.intervalSeconds = intervalFor(values.type, values.intervalSeconds, ordinaryInterval, false);
      }
      Object.assign(values, patch);
    });
  });
  // D-29: a new domain expiry check proposes 12 hours.
  $effect(() => {
    const { type } = values;
    untrack(() => {
      if (check === null) values.intervalSeconds = intervalFor(type, values.intervalSeconds, ordinaryInterval, touched.has('intervalSeconds'));
    });
  });
  let errors = $state<FieldError[]>([]);
  let saving = $state(false);
  let advancedOpen = $state(false);
  /** A check reads and saves its own channels apart from the check (NOT-003, NOT-005, F-3). */
  const channels = new ChannelsDraft(untrack(() => check?.id ?? null), untrack(() => contextChannelCalls()));
  let addedId = $state<string | null>(null);
  const resendId = `${formId}-resend`;

  const candidates = $derived(groupCandidates(check?.id ?? null, siblings));
  const warnings = $derived(check === null ? [] : secretWarnings(initial, values));
  const alerts = $derived(unplaced(errors, shownFields(values)));
  const error = (field: string) => messageFor(errors, field);

  function show(found: FieldError[]): void {
    errors = found;
    if (touchesAdvanced(found)) advancedOpen = true;
  }

  function createdId(value: unknown): string | null {
    if (typeof value !== 'object' || value === null || !('id' in value)) return null;
    return typeof value.id === 'string' ? value.id : null;
  }

  async function focusResend(): Promise<void> {
    await tick();
    document.getElementById(resendId)?.focus();
  }

  async function channelsFailed(created: boolean): Promise<void> {
    if (created && !channels.resendInvalid) channels.checkAdded();
    await focusResend();
  }

  async function retryChannels(id: string): Promise<void> {
    if ((await channels.save(id)) === 'failed') await channelsFailed(true);
    else onDone?.();
  }

  /** A new check saves its channels after it; if they fail, the next Save sends only the channels, never a second check. */
  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    if (saving) return;
    if (addedId === null) {
      const invalid = validateCheck(values, check === null ? null : initial);
      show(invalid);
      if (invalid.length > 0) return;
    }
    if (!channels.check()) {
      await focusResend();
      return;
    }
    saving = true;
    try {
      if (addedId !== null) {
        await retryChannels(addedId);
        return;
      }
      const sent = check === null ? 'skipped' : await channels.save(check.id);
      if (sent === 'failed') {
        await focusResend();
        return;
      }
      const result = await onSave($state.snapshot(values));
      if (!result.ok) {
        channels.checkNotSaved();
        show(placeHubErrors(fieldErrorsFrom(result.error), values));
        return;
      }
      const id = check === null ? createdId(result.value) : null;
      if (id !== null && (await channels.save(id)) === 'failed') {
        addedId = id;
        await channelsFailed(true);
        return;
      }
      onDone?.();
    } finally {
      saving = false;
    }
  }
</script>

<form id={formId} onsubmit={submit} novalidate>
  <fieldset class="check" disabled={addedId !== null}>
  <TextField label="Name" bind:value={values.name} error={error('name')} />

  {#if check === null}
    <SelectField label="Type" bind:value={values.type} options={TYPES} />
  {:else}
    <!-- CHK-009: the type is fixed once the check exists. -->
    <p class="type">Type: {TYPES.find((entry) => entry.value === values.type)?.label}</p>
  {/if}

  {#if values.type === 'HTTP'}
    <HttpFields bind:http={values.http} {errors} />
  {:else if values.type === 'GROUP'}
    <GroupFields {candidates} bind:childIds={values.group.childIds} error={error('childIds')} />
  {:else if values.type === 'DOMAIN_EXPIRY'}
    <DomainExpiryFields bind:domainExpiry={values.domainExpiry} {errors} />
  {:else if values.type === 'SNMP'}
    <SnmpFields bind:snmp={values.snmp} {errors} />
  {:else if values.type === 'DOCKER' || values.type === 'SYSTEMD' || values.type === 'TAILSCALE_PING'}
    <ProbeFields bind:values {errors} />
  {:else}
    <TypeFields bind:values {errors} />
  {/if}

  <Disclosure label="Advanced settings" bind:open={advancedOpen}>
    <AdvancedFields bind:values {errors} ontouch={(key) => touched.add(key)} />
  </Disclosure>
  </fieldset>

  {#if channels.phase !== 'absent'}
    <ChannelsSection draft={channels} {resendId} />
  {/if}

  {#if warnings.length > 0}
    <div class="warning" role="status">
      {#each warnings as warning (warning)}
        <p>{warning}</p>
      {/each}
    </div>
  {/if}

  <!-- Always mounted, so an error moves nothing (MOT-007). -->
  <div class="alert" role="alert">
    {#each alerts as found, index (index)}
      <p>{found.message}</p>
    {/each}
  </div>

  {#if footer === undefined}
    {@render actions()}
  {/if}
</form>

{#snippet actions()}
  <div class="actions">
    <PressButton label="Cancel" onclick={onCancel} />
    <PressButton label={addedId !== null ? 'Save channels' : check === null ? 'Add check' : 'Save check'} kind="primary" submit form={formId} busy={saving} />
  </div>
{/snippet}

<style>
  form {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    max-inline-size: calc(var(--space-16) * 8);
  }

  .check:disabled {
    opacity: var(--opacity-disabled);
  }

  .check {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-inline-size: 0;
    margin: 0;
    padding: 0;
    border: 0;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
  }

  .type {
    margin: 0;
    color: var(--text-secondary);
  }

  .alert {
    min-block-size: var(--target-min-desktop);
  }

  .alert:not(:empty),
  .warning {
    padding: var(--space-2) var(--space-3);
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
  }

  .alert:not(:empty) {
    background: var(--status-down-tint);
  }

  .warning {
    background: var(--status-warning-tint);
  }

  p {
    margin: 0;
  }
</style>
