<script lang="ts">
  import FindBar from '$lib/code/details/FindBar.svelte';
  import BrowserSurface, { type BrowserView } from '$lib/code/panel/browser/BrowserSurface.svelte';
  import { preset, FREE_VIEWPORT, type Viewport } from '$lib/code/panel/browser/viewport';

  interface Props {
    say: (what: string) => (...values: unknown[]) => void;
  }

  const { say }: Props = $props();
  const RECENT = [
    { url: 'http://localhost:5173/sign-in', title: 'Sign in · Anima' },
    { url: 'https://svelte.dev/docs/svelte/$state', title: '$state · Svelte docs' },
    { url: 'http://localhost:8080/health' },
  ];
  const SERVERS = [{ url: 'http://localhost:5173', process: 'vite' }, { url: 'http://localhost:8080', process: 'anima-hub' }];
  const noop = (): void => undefined;
  let url = $state('http://localhost:5173/sign-in');
  let view = $state<BrowserView>({ kind: 'page' });
  let viewport = $state<Viewport | undefined>(undefined);
  let zoom = $state(100);
  const bar = $derived({
    url,
    canBack: true,
    onback: say('back'),
    onforward: say('forward'),
    onreload: say('reload'),
    onstop: say('stop'),
    onopen: (next: string) => {
      url = next;
      view = { kind: 'page' };
    },
    onexternal: say('open in system browser'),
    more: { deviceToolbar: viewport !== undefined, appearance: 'system' as const, zoom, findKey: '⌘F' },
    onmore: (choice: string) => {
      if (choice === 'device_toolbar') viewport = viewport === undefined ? preset(FREE_VIEWPORT, 'iphone-se') : undefined;
      else if (choice === 'zoom_in') zoom += 25;
      else if (choice === 'zoom_out') zoom -= 25;
      else if (choice === 'zoom_reset') zoom = 100;
      else say('more')(choice);
    },
  });
  const still = $derived({ onback: noop, onforward: noop, onreload: say('reload'), onstop: noop, onopen: say('open'), onmore: say('more') });
  const more = { deviceToolbar: false, appearance: 'system' as const, zoom: 100 };
</script>

{#snippet page(size: Viewport | undefined)}
  <div class="stand-in">
    <strong>Sign in</strong>
    <span>{size === undefined ? 'The page fills the tab.' : `${String(size.width)} × ${String(size.height)}`}</span>
  </div>
{/snippet}

{#snippet find()}
  <FindBar words="Find in page" query="sign" find={{ kind: 'matches', index: 1, total: 3 }} onQuery={noop} onNext={say('next')} onPrevious={say('previous')} onRetry={noop} onClose={say('close find')} />
{/snippet}

<div class="scenes">
  <section data-kit="browser-live" aria-label="Browser, one that the controls change">
    <BrowserSurface {view} toolbar={bar} start={{ recent: RECENT, servers: SERVERS, onopen: bar.onopen }} {page} {viewport} onviewport={(next) => (viewport = next)} />
  </section>
  <section data-kit="browser-device" aria-label="Browser, device toolbar and find">
    <BrowserSurface view={{ kind: 'page' }} toolbar={{ ...still, url: 'http://localhost:5173/sign-in', loading: true, more: { ...more, deviceToolbar: true } }} start={{ onopen: noop }} {page} viewport={preset(FREE_VIEWPORT, 'iphone-se')} onviewport={say('viewport')} {find} />
  </section>
  <section data-kit="browser-start" aria-label="Browser, start page">
    <BrowserSurface view={{ kind: 'start' }} toolbar={{ ...still, url: '', more }} start={{ recent: RECENT, servers: SERVERS, onopen: say('open'), onforget: say('forget') }} {page} />
  </section>
  <section data-kit="browser-nothing" aria-label="Browser, nothing known">
    <BrowserSurface view={{ kind: 'start' }} toolbar={{ ...still, url: '', more }} start={{ onopen: noop }} {page} />
  </section>
  <section data-kit="browser-unreachable" aria-label="Browser, the site did not answer">
    <BrowserSurface view={{ kind: 'unreachable', detail: 'ERR_CONNECTION_REFUSED at http://localhost:5173/' }} toolbar={{ ...still, url: 'http://localhost:5173/', canBack: true, more }} start={{ onopen: noop }} {page} />
  </section>
</div>

<style>
  .scenes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-6);
  }

  section {
    display: flex;
    inline-size: calc(var(--space-16) * 8.5);
    block-size: calc(var(--space-16) * 6.5);
    overflow: hidden;
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
  }

  .stand-in {
    display: grid;
    flex: 1;
    place-content: center;
    gap: var(--space-2);
    color: var(--code-quiet-text);
    font-size: var(--code-text-small);
    text-align: center;
  }

  strong {
    color: var(--text);
    font-size: var(--code-text-body);
  }
</style>
