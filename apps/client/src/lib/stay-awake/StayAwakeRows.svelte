<script lang="ts">
  import { onMount } from 'svelte';
  import type { Level } from '$lib/desktop/stay-awake/shell';
  import FormButton from '$lib/forms/FormButton.svelte';
  import { useHub } from '$lib/hub/context';
  import { isMac } from '$lib/platform/mac';
  import SettingRow from '$lib/settings/SettingRow.svelte';
  import { describeStayAwake, floorChoices, hubIsThisComputer, LEVELS, sleepNotice } from './stay-awake';
  import { desktopStayAwakeShell, StayAwakeControl, type StayAwakeShell } from './stay-awake.svelte';

  interface Props {
    shell?: StayAwakeShell;
    /** A Mac has the set-up step of the lid-closed level (AWK-003). */
    mac?: boolean;
    /** The origin of the hub on this computer (AWK-005). */
    localHub?: string | null;
  }

  const { shell = desktopStayAwakeShell, mac = isMac(navigator.userAgent), localHub = null }: Props = $props();

  // The rows keep the shell they were mounted with.
  // svelte-ignore state_referenced_locally
  const control = new StayAwakeControl(shell);
  const hub = useHub();
  const thisComputerIsHub = $derived(hubIsThisComputer(hub.address.origin, localHub));

  // Once, not in an effect: `watch` reads and writes the control's own state.
  onMount(() => control.watch());

  const windows = /Windows NT/.test(navigator.userAgent);
  const status = $derived(control.status);
  const notice = $derived(status === null ? null : sleepNotice(thisComputerIsHub, status));
  const setUpRow = $derived(mac && status !== null);

  const NO_FLOOR = 'none';

  // The select shows the actual state, also after a refused change.
  async function choose(event: Event & { currentTarget: HTMLSelectElement }): Promise<void> {
    const select = event.currentTarget;
    await control.choose(select.value as Level);
    select.value = control.status?.level ?? 'off';
  }

  async function setFloor(event: Event & { currentTarget: HTMLSelectElement }): Promise<void> {
    const select = event.currentTarget;
    await control.setFloor(select.value === NO_FLOOR ? null : Number(select.value));
    select.value = String(control.status?.batteryFloor ?? NO_FLOOR);
  }
</script>

<!-- AWK-001; a choice saves at once (SET-005). -->
<SettingRow label="Stay awake" for="stay-awake-level">
  {#snippet hint()}
    <p class="hint state" aria-live="polite">
      {#if status !== null}
        {describeStayAwake(status)}
        {#if status.lid === 'unsupported'}
          {#if windows}
            Closed-lid sleep control is unavailable on this computer.
          {:else}
            This desktop controls the closed lid. Change it in power settings.
          {/if}
        {/if}
      {/if}
    </p>
    {#if thisComputerIsHub}
      <!-- Always rendered: its line is reserved (MOT-007) and new text is announced. -->
      <p class="hint notice" class:shown={notice !== null} role="status">{notice ?? ''}</p>
    {/if}
  {/snippet}
  <select id="stay-awake-level" value={status?.level ?? 'off'} disabled={status === null || control.busy} onchange={(event) => void choose(event)}>
    {#each LEVELS as level (level.value)}
      <option value={level.value} disabled={level.value === 'lid-closed' && status?.lid !== 'ready'}>{level.label}</option>
    {/each}
  </select>
  {#snippet message()}
    <p role="alert">{control.problem ?? ''}</p>
  {/snippet}
</SettingRow>

{#if setUpRow}
  <!-- The one set-up step that the operator starts; macOS asks for the administrator password (AWK-003, INS-009). -->
  <SettingRow label="Lid closed set-up">
    {#snippet hint()}
      <p class="hint state">
        {#if status?.lid === 'ready'}
          Set up. This user's programs can change one sleep setting of this Mac without a password, nothing else.
        {:else}
          macOS asks for an administrator once. Then this user's programs can change one sleep setting of this Mac
          without a password, nothing else.
        {/if}
      </p>
    {/snippet}
    {#if status?.lid === 'ready'}
      <FormButton size="sm" busy={control.busy} onclick={() => void control.removeLidSetUp()}>Remove</FormButton>
    {:else}
      <FormButton size="sm" busy={control.busy} onclick={() => void control.setUpLid()}>Set up</FormButton>
    {/if}
  </SettingRow>
{/if}

<!-- AWK-006 -->
<SettingRow label="On battery" for="stay-awake-floor">
  {#snippet hint()}
    <p class="hint">Below this charge, off until mains power returns.</p>
  {/snippet}
  <select id="stay-awake-floor" value={status?.batteryFloor ?? NO_FLOOR} disabled={status === null || control.busy} onchange={(event) => void setFloor(event)}>
    <option value={NO_FLOOR}>Any charge</option>
    {#each floorChoices(status?.batteryFloor ?? null) as floor (floor)}
      <option value={floor}>Off below {floor}%</option>
    {/each}
  </select>
</SettingRow>

<style>
  /* Two lines are reserved, so a longer sentence does not move the rows below it (MOT-007). */
  .state {
    min-block-size: calc(2 * var(--text-sm) * var(--leading-tight));
  }

  /* The warning keeps the text colour; a bar marks it, as the row's error line does (MAT-005). */
  .notice {
    padding-inline-start: var(--space-2);
    border-inline-start: var(--border-focus) solid transparent;
    color: var(--text);
  }

  .notice.shown {
    border-inline-start-color: var(--status-warning);
  }
</style>
