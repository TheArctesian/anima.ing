<script lang="ts">
  import type { Snippet } from 'svelte';
  import CodeIcon from '../icons/CodeIcon.svelte';
  import Automations from './related/Automations.svelte';
  import BranchRow from './version/BranchRow.svelte';
  import Control from './row/Control.svelte';
  import './details.css';
  import IssueRow from './version/IssueRow.svelte';
  import { fits, shows, type Density, type ItemChoices } from './items';
  import ItemsMenu from './ItemsMenu.svelte';
  import Lineage from './related/Lineage.svelte';
  import { runningHeading } from './labels';
  import Meters from './meters/Meters.svelte';
  import type { Details, Ref } from './model';
  import MoreButton from './row/MoreButton.svelte';
  import OpenInRow from './workspace/OpenInRow.svelte';
  import PullRequestRow from './version/PullRequestRow.svelte';
  import RelatedList from './related/RelatedList.svelte';
  import RunContextRow from './workspace/RunContextRow.svelte';
  import ScriptsRow from './workspace/ScriptsRow.svelte';
  import Section from './row/Section.svelte';
  import Servers from './related/Servers.svelte';
  import type { Actions } from './actions';

  interface Props {
    details: Details;
    /** The items that the person switched; the owner keeps them (`keepChoices`). */
    choices: ItemChoices;
    onChoices: (choices: ItemChoices) => void;
    /** How much the card shows; the owner picks it by the room it has (`densityFor`). */
    density?: Density;
    /** What the engine can do; a control whose action is absent is off. */
    on?: Actions;
    /** The refs of the picker, loaded when it opens. */
    refs?: readonly Ref[];
    refsLoading?: 'first' | 'more' | undefined;
    refsTotal?: number | undefined;
    /** The Git rows of the Git stage (the quick action, "Changes", the repositories) in Version Control. */
    git?: Snippet<[Density]> | undefined;
  }

  const { details, choices, onChoices, density = 'full', on = {}, refs = [], refsLoading, refsTotal, git }: Props = $props();
  const show = $derived({
    run: details.runContext !== undefined && shows(choices, 'environment') && fits(density, 'runContext'),
    openIn: details.openIn !== undefined && shows(choices, 'openIn') && fits(density, 'openIn'),
    scripts: details.scripts !== undefined && shows(choices, 'scripts'),
    branch: shows(choices, 'branch'),
    git: git !== undefined && (shows(choices, 'gitActions') || shows(choices, 'changes')),
    related: details.draft !== true && fits(density, 'related'),
  });
  const requests = $derived(show.branch ? (details.pullRequests ?? []) : []);
  const issues = $derived(show.branch ? (details.issues ?? []) : []);
  const servers = $derived(show.related && shows(choices, 'servers') ? (details.servers ?? []) : []);
  const automations = $derived(show.related && shows(choices, 'automations') ? (details.automations ?? []) : []);
  const automationsError = $derived(show.related && shows(choices, 'automations') ? details.automationsError : undefined);
  const lineage = $derived(show.related && shows(choices, 'lineage') ? (details.related ?? []).filter((row) => row.kind !== 'side_chat') : []);
  const sideChats = $derived(show.related && shows(choices, 'sideChats') ? (details.related ?? []).filter((row) => row.kind === 'side_chat') : []);
  const usage = $derived(shows(choices, 'usage') ? details.usage : undefined);
  const context = $derived(shows(choices, 'context') ? details.context : undefined);
  const workspace = $derived(show.run || show.openIn || show.scripts);
  const branch = $derived(show.branch ? details.branch : undefined);
  const version = $derived(branch !== undefined || requests.length > 0 || issues.length > 0 || show.git);
  const empty = $derived(!workspace && !version && servers.length + automations.length + lineage.length + sideChats.length === 0 && automationsError === undefined && usage === undefined && context === undefined);
  let allRequests = $state(false);
  let allIssues = $state(false);
  let menu = $state<{ left: number; top: number; width: number; height: number }>();
  let chooser = $state<HTMLButtonElement>();

  /** A right-click on the card opens the menu of its items; a field, a link and selected text keep their own menu. */
  function oncontextmenu(event: MouseEvent): void {
    const target = event.target instanceof Element ? event.target : null;
    if (target?.closest('input, textarea, a, [contenteditable]') != null || (getSelection()?.toString() ?? '') !== '') return;
    event.preventDefault();
    menu = { left: event.clientX, top: event.clientY, width: 0, height: 0 };
  }
</script>

<!--
  The details card of a thread (02 section 7; 07): Workspace, Version Control, Servers, Automations, Lineage, Side
  chats, the limits and the context window. A part with no content is not drawn. Opaque: no glass.
-->
<aside class="details-card" aria-label="Thread details" {oncontextmenu}>
  {#if workspace}
    <Section label="Workspace" separated={false}>
      {#if show.run && details.runContext !== undefined}<RunContextRow context={details.runContext} onEnvironment={on.environment} onMode={on.mode} onCopyPath={on.copy} />{/if}
      {#if show.openIn && details.openIn !== undefined}<OpenInRow openIn={details.openIn} shortcut={on.openShortcut} onOpen={on.openEditor} onPrefer={on.preferEditor} />{/if}
      {#if show.scripts && details.scripts !== undefined}<ScriptsRow scripts={details.scripts} onRun={on.runScript} onAdd={on.addScript} onEdit={on.editScript} />{/if}
    </Section>
  {/if}
  {#if version}
    <Section label="Version Control" separated={workspace && density === 'full'}>
      {#if branch !== undefined}<BranchRow {branch} {refs} loading={refsLoading} total={refsTotal} onOpen={on.loadRefs} onPick={on.pickRef} onCreate={on.createRef} onFromRemote={on.fromRemote} onCopy={on.copy} />{/if}
      {#each allRequests ? requests : requests.slice(0, 1) as request (request.number)}
        <PullRequestRow {request} onOpen={on.openPullRequest} onUnwatch={on.unwatch} onChecks={on.openChecks} onAction={on.pullRequestAction} />
      {/each}
      {#if requests.length > 1}<MoreButton more={allRequests ? 0 : requests.length - 1} expanded={allRequests} onclick={() => (allRequests = !allRequests)} />{/if}
      {#each allIssues ? issues : issues.slice(0, 1) as issue (issue.key)}
        <IssueRow {issue} onOpen={on.openIssue} />
      {/each}
      {#if issues.length > 1}<MoreButton more={allIssues ? 0 : issues.length - 1} expanded={allIssues} onclick={() => (allIssues = !allIssues)} />{/if}
      {#if show.git}{@render git?.(density)}{/if}
    </Section>
  {/if}
  {#if servers.length > 0}<Servers {servers} modifier={on.modifier} onOpen={on.openServer} onForward={on.forward} onStop={on.stopServer} busy={on.busyServer} />{/if}
  {#if automations.length > 0 || automationsError !== undefined}
    <Automations {automations} error={automationsError} busy={on.busyAutomation} onManage={on.manageAutomations} onEdit={on.editAutomation} onRun={on.runAutomation} onEnable={on.enableAutomation} />
  {/if}
  {#if lineage.length > 0}<Lineage rows={lineage} onOpen={on.openThread} onStop={on.stopAgent} stopping={on.stoppingAgent} onDisconnect={on.disconnect} />{/if}
  {#if sideChats.length > 0}
    <Section label="Side chats" heading={runningHeading('Side chats', sideChats)}>
      <RelatedList rows={sideChats} onOpen={on.openThread} />
    </Section>
  {/if}
  <Meters {usage} {context} onUsage={on.openUsage} />
  {#if empty}
    <div class="choose">
      <Control still aria-haspopup="menu" aria-expanded={menu !== undefined} bind:element={chooser} onclick={() => (menu = chooser?.getBoundingClientRect())}>
        <CodeIcon name="sliders-horizontal" />
        <span class="details-label">Choose what to show</span>
      </Control>
    </div>
  {/if}
</aside>
{#if menu !== undefined}
  {@const at = menu}
  <ItemsMenu
    {choices}
    anchor={() => at}
    from={chooser}
    onChange={onChoices}
    onClose={(back) => {
      menu = undefined;
      if (back) chooser?.focus();
    }}
  />
{/if}

<style>
  .details-card {
    display: flex;
    flex-direction: column;
    inline-size: var(--details-card);
    max-inline-size: 100%;
    max-block-size: 100%;
    overflow: hidden auto;
    border: var(--border-thin) solid var(--border-strong);
    border-radius: var(--radius-lg);
    background: var(--surface-overlay);
    box-shadow: var(--elevation-2);
    color: var(--text);
  }

  /* The first section that shows has no line above it. */
  .details-card > :global(section:first-child) {
    border-block-start: 0;
  }

  .choose {
    display: flex;
    padding: var(--details-section-pad);
  }
</style>
