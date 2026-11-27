<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { sendPreviewHandoff } from '$lib/desktop/web-view/workspace';
  import { desktopWebView } from '$lib/desktop/web-view/web-view';
  import { overlayCovers } from '$lib/overlay/overlay.svelte';
  import { boundsOf, hasArea, nativeView, type ViewState } from '$lib/web-interfaces/tab/view';
  import type { Bounds } from '$lib/web-interfaces/shell/shell';
  import type { Handoff } from './browser';
  import IconButton from '../../kit/IconButton.svelte';
  import Tooltip from '../../kit/Tooltip.svelte';

  const { device, endpoint, handoff, onclose }: { device: string; endpoint: string; handoff: Handoff; onclose: () => void } = $props();
  const tabId = `code-preview-${crypto.randomUUID()}`;
  const launch = untrack(() => `${new URL(handoff.action).origin}/.anima/connect`);
  let area: HTMLDivElement | undefined = $state();
  let bounds = $state<Bounds | undefined>();
  let listening = $state(false);
  let viewState = $state<ViewState>('none');
  let error = $state('');
  let pageShown = $state(true);
  let sent = false;
  let opened = false;
  let alive = false;
  const view = nativeView({ shell: desktopWebView, tabId, onstate: (next) => { viewState = next; }, onfail: () => { error = 'The preview view could not connect. Return to Code and prepare it again.'; } });

  function measure(): void {
    if (!area) return;
    bounds = boundsOf(area.getBoundingClientRect());
    if (hasArea(bounds)) view.place(bounds);
  }
  onMount(() => {
    alive = true;
    pageShown = !document.hidden;
    let unlisten: (() => void) | undefined;
    const timer = setTimeout(() => { if (alive && !sent) error = 'The preview took too long to open. Prepare it again.'; }, 15_000);
    void desktopWebView.listen((event) => {
      if (!alive || event.tabId !== tabId) return;
      if (event.kind === 'FAILED' || event.kind === 'UNTRUSTED' || event.kind === 'MISMATCH') {
        error = 'The preview connection or certificate could not be verified.';
      } else if (event.kind === 'LOADED' && event.url === launch && !sent) {
        sent = true;
        clearTimeout(timer);
        if (handoff.expires <= Date.now()) { error = 'Prepare a fresh preview link.'; return; }
        void sendPreviewHandoff(tabId, { endpoint, action: handoff.action, token: handoff.token })
          .catch(() => { if (alive) error = 'The preview handoff failed. Prepare it again.'; });
      }
    }).then((stop) => {
      if (!alive) { stop(); return; }
      unlisten = stop;
      listening = true;
      measure();
    }).catch(() => { if (alive) error = 'The preview view is unavailable.'; });
    const observer = new ResizeObserver(() => { if (unlisten) measure(); });
    if (area) observer.observe(area);
    window.addEventListener('resize', measure);
    window.addEventListener('scroll', measure, true);
    return () => {
      alive = false; clearTimeout(timer); unlisten?.(); observer.disconnect();
      window.removeEventListener('resize', measure); window.removeEventListener('scroll', measure, true);
      view.restart(); view.close();
    };
  });
  $effect(() => {
    if (listening && bounds && hasArea(bounds) && !opened && !error) {
      opened = true;
      view.open({ deviceId: device, url: launch, pin: null, previewEndpoint: endpoint }, bounds);
    }
  });
  $effect(() => { if (viewState === 'open') view.show(pageShown && !overlayCovers() && !error); });
</script>

<svelte:document onvisibilitychange={() => (pageShown = !document.hidden)} />
<section class="native-preview" aria-label="Workspace preview">
  <header>
    <button class="node-secondary" type="button" onclick={onclose}>Conversation</button>
    <span>Workspace preview</span>
    <Tooltip text="Reload preview" side="below"><IconButton label="Reload preview" icon="refresh-cw" disabled={viewState !== 'open'} onclick={() => view.go('RELOAD')} /></Tooltip>
    <Tooltip text="Move preview to a window" side="below"><IconButton label="Move preview to a window" icon="external-link" disabled={viewState !== 'open'} onclick={() => view.popOut()} /></Tooltip>
  </header>
  <div class="preview-area" bind:this={area}>
    {#if error}<p role="alert">{error}</p>{:else if viewState === 'popped'}<p>The preview is open in its own window.</p>{:else}<p role="status">Opening private preview…</p>{/if}
  </div>
</section>

<style>
  .native-preview { display:flex; flex-direction:column; flex:1; min-block-size:0; }
  header { display:flex; align-items:center; flex-wrap:wrap; gap:var(--space-2); padding-block-end:var(--space-3); }
  header span { flex:1; font-size:var(--text-sm); }
  .preview-area { flex:1; min-block-size:16rem; overflow:hidden; border:1px solid var(--border-subtle); border-radius:var(--radius-md); }
  p { padding:var(--space-4); font-size:var(--text-sm); color:var(--text-secondary); }
</style>
