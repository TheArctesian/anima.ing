<script lang="ts">
  import '$lib/code/kit/code.css';
  import DetailsCard from '$lib/code/details/DetailsCard.svelte';
  import FindBar from '$lib/code/details/FindBar.svelte';
  import type { ShownChoices } from '$lib/code/details/shown';
  import LibraryDiff from '$lib/code/panel/diff/LibraryDiff.svelte';
  import FileSurface from '$lib/code/panel/files/FileSurface.svelte';
  import FilesSurface from '$lib/code/panel/files/FilesSurface.svelte';
  import PanelToggles from '$lib/code/panel/PanelToggles.svelte';
  import RightPanel from '$lib/code/panel/RightPanel.svelte';
  import { closed, shown, type PanelTabs } from '$lib/code/panel/tabs';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import BrowserScenes from './BrowserScenes.svelte';
  import DiffScenes from './DiffScenes.svelte';
  import LibraryTree from '$lib/code/panel/tree/LibraryTree.svelte';
  import TerminalScenes from './TerminalScenes.svelte';
  import TreeScenes from './TreeScenes.svelte';
  import { PROJECT_PATHS } from './diff-scenes';
  import { FULL, MARKDOWN, PATCH, PICTURE, PLAIN, REFS, SOURCE, TABS, listing } from './scenes';

  let choices = $state<ShownChoices>({ usage: true, context: true });
  let said = $state('none');
  let tabs = $state<PanelTabs>(TABS);
  let width = $state(640);
  let query = $state('');
  let find = $state('redirect');
  let parts = $state({ details: true, terminal: false, panel: true });
  let maximised = $state(false);
  const say =
    (what: string) =>
    (...values: unknown[]): void => {
      said = `${what} ${values.map((value) => (typeof value === 'object' ? JSON.stringify(value) : String(value))).join(' ')}`;
    };
  const on = {
    modifier: '⌘',
    openShortcut: '⌘O',
    copy: say('copy'),
    environment: say('environment'),
    mode: say('mode'),
    openEditor: say('open editor'),
    preferEditor: say('prefer editor'),
    runScript: say('run script'),
    addScript: say('add script'),
    editScript: say('edit script'),
    pickRef: say('pick ref'),
    createRef: say('create ref'),
    openPullRequest: say('open pull request'),
    unwatch: say('unwatch'),
    openChecks: say('checks'),
    pullRequestAction: say('pull request action'),
    openIssue: say('open issue'),
    openServer: say('open server'),
    forward: say('forward'),
    stopServer: say('stop server'),
    manageAutomations: say('manage automations'),
    editAutomation: say('edit automation'),
    runAutomation: say('run automation'),
    enableAutomation: say('enable automation'),
    openThread: say('open thread'),
    stopAgent: say('stop agent'),
    disconnect: say('disconnect'),
    openUsage: say('open usage'),
  };
  const noop = (): void => undefined;
  let drawn = $state('loading');
  let dark = $state(true);
  $effect(() => {
    const root = document.documentElement;
    const read = (): void => void (dark = root.dataset['theme'] === 'dark' || (root.dataset['theme'] === undefined && matchMedia('(prefers-color-scheme: dark)').matches));
    read();
    const watch = new MutationObserver(read);
    watch.observe(root, { attributes: true, attributeFilter: ['data-theme'] });
    return () => watch.disconnect();
  });
</script>

{#snippet tree()}
  <LibraryTree label="anima files" paths={PROJECT_PATHS} marks={[{ path: 'node_modules/', status: 'ignored' }]} compact {query} theme={dark ? 'dark' : 'light'} selected="src/routes/sign-in/+page.svelte" onpick={say('open file')} />
{/snippet}

<!-- Stage 5 of the T3 Code copy: the details card, the find bar and the right panel with its file surfaces, for the gallery. -->
<div class="kit">
  <PageHeader title="Code kit: details and panel" detail="T3 Code's details card and right panel in Anima's skin" eyebrow={{ text: 'Stage 5', icon: 'carbon:code' }} />
  <div class="cards">
    <section data-kit="details-full" aria-label="Details card, full"><DetailsCard details={FULL} {choices} onChoices={(next) => (choices = next)} {on} refs={REFS} /></section>
    <section data-kit="details-compact" aria-label="Details card, compact"><DetailsCard details={FULL} {choices} onChoices={(next) => (choices = next)} {on} density="compact" /></section>
    <section data-kit="details-essential" aria-label="Details card, essential"><DetailsCard details={FULL} {choices} onChoices={(next) => (choices = next)} {on} density="essential" /></section>
    <section data-kit="details-plain" aria-label="Details card, no engine"><DetailsCard details={PLAIN} choices={{ usage: true, context: true }} onChoices={noop} /></section>
    <section data-kit="details-empty" aria-label="Details card, empty"><DetailsCard details={{}} {choices} onChoices={(next) => (choices = next)} /></section>
    <section data-kit="toggles" aria-label="Panel toggles">
      <PanelToggles open={parts} {maximised} keys={{ details: '⇧⌘B', terminal: '⌘J', panel: '⌥⌘B' }} ontoggle={(part) => (parts = { ...parts, [part]: !parts[part] })} onmaximise={() => (maximised = !maximised)} />
      <PanelToggles open={{ details: false, terminal: false, panel: false }} project={false} ontoggle={noop} />
    </section>
    <section data-kit="find" aria-label="Find bar">
      <FindBar query={find} find={{ kind: 'matches', index: 3, total: 17 }} onQuery={(next) => (find = next)} onNext={say('next')} onPrevious={say('previous')} onRetry={noop} onClose={say('close find')} />
      <FindBar query="zzz" find={{ kind: 'matches', index: 0, total: 0 }} onQuery={noop} onNext={noop} onPrevious={noop} onRetry={noop} onClose={noop} />
      <FindBar query="login" find={{ kind: 'failed' }} onQuery={noop} onNext={noop} onPrevious={noop} onRetry={say('retry')} onClose={noop} />
    </section>
  </div>
  <section class="panel" data-kit="panel" aria-label="Right panel">
    <RightPanel open state={tabs} {width} onresize={(next) => (width = next)} onshow={(id) => (tabs = shown(tabs, id))} onclose={(id) => (tabs = closed(tabs, id))} onopen={say('open surface')} onadd={say('add surface')}>
      {#snippet surface(tab)}
        {#if tab.kind === 'files'}
          <FilesSurface project="anima" {query} onquery={(next) => (query = next)} onrefresh={say('refresh')} listed folds="closed" onfold={say('fold')} {tree} />
        {:else}
          <FileSurface project="anima" path={tab.path ?? ''} file={{ state: 'ready', text: SOURCE }} {listing} onopen={say('open file')} onnavigate={say('navigate')} line={6} {tree} />
        {/if}
      {/snippet}
    </RightPanel>
  </section>
  <div class="files">
    <section data-kit="file-markdown" aria-label="File, markdown"><FileSurface project="anima" path="docs/sign-in.md" file={{ state: 'ready', text: MARKDOWN }} rendered {listing} onopen={noop} onnavigate={noop} explorerOpen={false} sheet /></section>
    <section data-kit="file-image" aria-label="File, image"><FileSurface project="anima" path="static/mark.svg" file={{ state: 'ready', src: PICTURE }} {listing} onopen={noop} onnavigate={noop} explorerOpen={false} sheet /></section>
    <section data-kit="file-error" aria-label="File, read error"><FileSurface project="anima" path="src/missing.ts" file={{ state: 'failed', failure: 'operation_failed' }} cwd="/Users/ada/anima" {listing} onopen={noop} onnavigate={noop} onretry={say('retry')} explorerOpen={false} sheet /></section>
    <section data-kit="file-readonly" aria-label="File, read-only and cut"><FileSurface project="anima" path="logs/hub.log" file={{ state: 'ready', text: SOURCE, truncated: true, bytes: 4_200_000 }} readOnly {listing} onopen={noop} onnavigate={noop} explorerOpen={false} sheet /></section>
    <section data-kit="files-error" aria-label="Files, read error"><FilesSurface project="anima" query="" onquery={noop} onrefresh={noop} error="The workspace folder could not be read." onretry={noop} sheet {tree} /></section>
    <section data-kit="files-no-match" aria-label="Files, no match"><FilesSurface project="anima" query="zzz" onquery={noop} onrefresh={noop} noMatch sheet {tree} /></section>
  </div>
  <div class="files">
    <section data-kit="diff-stacked" aria-label="Diff library, stacked"><LibraryDiff patch={PATCH} theme={dark ? 'dark' : 'light'} ondrawn={(failure) => (drawn = failure ?? 'drawn')} /></section>
    <section data-kit="diff-split" aria-label="Diff library, split"><LibraryDiff patch={PATCH} split wrap theme={dark ? 'dark' : 'light'} /></section>
  </div>
  <DiffScenes theme={dark ? 'dark' : 'light'} {say} />
  <TreeScenes theme={dark ? 'dark' : 'light'} {say} />
  <TerminalScenes {say} />
  <BrowserScenes {say} />
  <p class="said" data-kit-drawn={drawn}>Diff library: {drawn}</p>
  <p class="said" aria-live="polite">Last action: {said}</p>
</div>

<style>
  .kit {
    display: grid;
    gap: var(--space-10);
    padding-block-end: var(--space-16);
  }

  .cards,
  .files {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-start;
    gap: var(--space-6);
  }

  .cards section {
    display: grid;
    gap: var(--space-2);
    inline-size: var(--details-card);
  }

  .panel,
  .files section {
    display: flex;
    block-size: 30rem;
    overflow: hidden;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    background: var(--surface-base);
  }

  .panel {
    justify-content: flex-end;
  }

  .files section {
    flex-direction: column;
    inline-size: 34rem;
    block-size: 22rem;
  }

  .said {
    margin: 0;
    padding: var(--space-3);
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }
</style>
