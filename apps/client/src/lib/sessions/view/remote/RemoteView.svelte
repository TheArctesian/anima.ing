<script lang="ts">
  import { untrack } from 'svelte';
  import { capabilities } from '$lib/platform/platform';
  import { isMobile } from '$lib/platform/mobile';
  import type { RemoteProtocol, SessionsApi } from '../../api';
  import type { SessionEnding } from '../../ending';
  import { receiveClipboard } from '../../tunnel/guacamole/input';
  import type { GuacamoleState, SessionTunnel } from '../../tunnel/guacamole/tunnel';
  import { displayInput, type Size } from '../display';
  import EndedPanel from '../EndedPanel.svelte';
  import { keyRoute, pasteText, sendCombination } from '../guacamole/input';
  import CertificateModal from '../certificate/CertificateModal.svelte';
  import CredentialPrompt from '../credentials/CredentialPrompt.svelte';
  import { keyFilter, type KeyCombination, type KeyFilter } from '../guacamole/keys';
  import { REMOTE_CONFLICT, SessionConnection, progressOf, type Stage } from '../session.svelte';
  import type { RemoteStart } from '../start';
  import { clipboardRules, NO_CLIPBOARD } from '../clipboard/clipboard';
  import ConnectionPanel from '../steps/ConnectionPanel.svelte';
  import { playStartFeedback } from '../steps/feedback.svelte';
  import { displayScale, fillDisplay } from './fit';
  import { followPixelRatio, remotePane, type RemotePane } from './pane';
  import { gatewayAsk, PROMPT_CANCELLED, PROMPT_REFUSED } from './prompt';
  import RemoteToolbar from './RemoteToolbar.svelte';
  import RemoteDisplay from './RemoteDisplay.svelte';

  type Client = SessionTunnel['client'];

  interface Props {
    device: { readonly id: string; readonly name: string };
    protocol: RemoteProtocol;
    /** Starts a session on `device` with a new ticket; see `sessionStart(...).remote(protocol)` in `../start`. */
    start: RemoteStart;
    api: Pick<SessionsApi, 'endSession' | 'setCredential' | 'testConnection' | 'acceptServerCertificate'>;
    /** The pane is on screen (its tab is selected). */
    visible: boolean;
    /** The pane should take the keyboard (its tab is selected and the app has focus). */
    focused: boolean;
    onback?: (() => void) | undefined;
    /** Hears the final ending of each attempt (after `endSession` answered, D-4). */
    onended?: (ending: SessionEnding) => void;
    /** Hears the client of each opened tunnel before the gateway's first frame; the function it returns runs when that tunnel goes. */
    onclient?: (client: Client) => (() => void) | undefined;
  }

  const { device, protocol, start, api, visible, focused, onback, onended, onclient }: Props = $props();
  /** Fixed texts of the clipboard notices; never the clipboard's content (LOG-007). */
  const COPY_FAILED = 'Could not copy to your clipboard.';
  const PASTE_FAILED = 'Could not read your clipboard. Allow access, or use Paste in the Keys menu.';
  const PASTE_REFUSED = 'Paste to the device is off. Nothing sent.';

  const connection = new SessionConnection<SessionTunnel, GuacamoleState>(
    (display, events) => start(display, events),
    { endSession: (id) => api.endSession(id) },
    (ending) => onended?.(ending),
    untrack(() => REMOTE_CONFLICT[protocol]),
  );
  let pane: RemotePane | undefined;
  /** The remote size the open tunnel started with, in device pixels. */
  let startedAt: Size = { width: 0, height: 0 };
  let paneElement: HTMLElement | undefined = $state();
  /** The pane's size in CSS px; read by the display scale, so it is state. */
  let paneSize: Size | undefined = $state();
  /** The remote display's own size, as the gateway last set it. */
  let remote: Size = $state({ width: 0, height: 0 });
  /** What the device holds and which keys reach it, for the open tunnel; the paste reads it too. */
  let filter: KeyFilter = keyFilter({ paste: false });
  let notice: string | undefined = $state();
  /** Whether the keyboard focus is inside the pane, and the window has it (D-8). */
  let paneHasFocus = $state(false);
  /** `devicePixelRatio`, followed as state so the display scale and the RDP size follow a monitor change. */
  let pixelRatio = $state(window.devicePixelRatio);

  /** The `required` instruction the operator answered; it asks nothing more (GW-015). */
  let answered: readonly string[] | undefined = $state.raw();
  /** The ended attempt whose certificate warning was closed or answered. */
  let dismissed: Stage<SessionTunnel, GuacamoleState> | undefined = $state.raw();

  const mobile = capabilities().deviceLink && isMobile(navigator);
  let displayView: RemoteDisplay | undefined = $state();
  const stage = $derived(connection.stage);
  const client = $derived(connection.tunnel?.client);
  const ready = $derived(connection.ready);
  playStartFeedback(() => progressOf(connection.stage));
  const displayState = $derived(protocol === 'VNC' && stage.kind === 'open' ? stage.state : undefined);
  const rules = $derived(stage.kind === 'open' ? clipboardRules(stage.policy) : NO_CLIPBOARD);
  const ask = $derived(gatewayAsk(protocol, stage.kind === 'open' ? stage.state.required : undefined, answered));
  /** The parent gives this pane the keyboard, unless the sign-in prompt has it; the pane then takes the focus. */
  const wanted = $derived(visible && focused && ready && ask.kind !== 'ask');
  /** The sign-in prompt to show, at most one; a hidden tab shows none. */
  const prompts = $derived(visible && ask.kind === 'ask' && client !== undefined ? [{ ...ask, client }] : []);
  // A hidden tab's prompt or warning waits until its tab is shown, so it never covers or traps another tab (SEC-007).
  const certificateOpen = $derived(visible && stage.kind === 'ended' && stage.certificateChanged && dismissed !== stage);
  /** Keys are sent only while the focus is really in the pane, so no key is held when it leaves (D-8). */
  const bound = $derived(wanted && paneHasFocus);
  const scale = $derived(onback ? fillDisplay(remote, paneSize ?? remote) : displayScale(protocol, remote, paneSize ?? remote, pixelRatio));

  function connect(css: Size) {
    const display = displayInput(css, window.devicePixelRatio, Intl.DateTimeFormat().resolvedOptions().timeZone);
    startedAt = { width: display.widthPx, height: display.heightPx };
    void connection.connect(display);
  }

  /** Show the certificate warning again, which runs a new connection test (K4 §3). */
  /** The ended panel, so that the certificate warning gives focus back to its button. */
  let ended: EndedPanel | undefined = $state();

  function checkCertificate() {
    dismissed = undefined;
  }

  function reconnect() {
    dismissed = stage;
    notice = undefined;
    connect(paneSize ?? { width: 0, height: 0 });
  }

  function copyToSystem(text: string) {
    navigator.clipboard.writeText(text).catch(() => {
      notice = COPY_FAILED;
    });
  }

  /** The operator's paste shortcut (Command+V, or Ctrl+V and the others the filter leaves to the browser). */
  function paste(event: ClipboardEvent) {
    if (client === undefined || !ready) return;
    event.preventDefault();
    if (!rules.paste) notice = PASTE_REFUSED;
    else pasteText(client, event.clipboardData?.getData('text/plain') ?? '', filter.held);
  }

  function sendKeys(combination: KeyCombination) {
    if (client !== undefined) sendCombination(client, combination);
    paneElement?.focus({ preventScroll: true });
  }

  function pasteFromClipboard() {
    const target = client;
    if (target === undefined || !rules.paste) return;
    navigator.clipboard.readText().then(
      (text) => {
        if (target === client && rules.paste) pasteText(target, text, filter.held);
        paneElement?.focus({ preventScroll: true });
      },
      () => {
        notice = PASTE_FAILED;
      },
    );
  }

  /** The pane's keyboard, focus and size: the first measure starts the session, later ones resize it (D-5). */
  function attachPane(element: HTMLElement) {
    paneElement = element;
    const attached = remotePane(element, {
      focus: (inside) => {
        // Chromium fires `focusout` while Svelte removes the pane, inside its block effect, where a tracked state
        // write is forbidden (`state_unsafe_mutation`); this handler reads nothing, so the write is untracked.
        untrack(() => {
          paneHasFocus = inside;
        });
      },
      measured: (css) => {
        const first = paneSize === undefined;
        paneSize = css;
        if (first) connect(css);
      },
      timer: {
        set: (callback, ms) => setTimeout(callback, ms),
        clear: (handle) => {
          clearTimeout(handle as ReturnType<typeof setTimeout>);
        },
      },
    });
    pane = attached;
    return () => {
      attached.stop();
      pane = undefined;
    };
  }

  // Each open tunnel gets a key filter for its policy, its clipboard and display hooks and, for RDP, a size sender.
  $effect(() => {
    if (client === undefined) return;
    const target = client;
    const allowed = untrack(() => rules);
    filter = keyFilter({ paste: allowed.paste });
    const display = target.getDisplay();
    remote = { width: display.getWidth(), height: display.getHeight() };
    display.onresize = (width, height) => {
      remote = { width, height };
    };
    // Text the gateway sends while copy is off, or while the pane is not bound, is dropped: a background session
    // never writes the system clipboard (RA-033, D-8).
    receiveClipboard(target, (text) => {
      if (allowed.copy && bound) copyToSystem(text);
    });
    const gone = untrack(() => onclient?.(target));
    const sizes = protocol === 'RDP' ? pane?.sendSizes(target, startedAt) : undefined;
    return () => {
      sizes?.();
      gone?.();
      display.onresize = null;
      target.onclipboard = null;
    };
  });

  $effect(() =>
    followPixelRatio((ratio) => {
      pixelRatio = ratio;
      pane?.remeasure();
    }),
  );

  $effect(() => {
    if (wanted) paneElement?.focus({ preventScroll: true });
  });

  // The keyboard is bound only while the pane is visible, focused and ready and holds the DOM focus; never to the
  // document (D-8). Unbinding releases every held key on the device.
  $effect(() => {
    if (!bound || client === undefined || pane === undefined) return;
    const keyboard = pane.keyboard;
    keyboard.bind(keyRoute(client, filter));
    return () => {
      keyboard.unbind();
    };
  });

  // A `required` the prompt cannot answer would leave the gateway waiting: end the session (it fails closed).
  $effect(() => {
    if (ask.kind === 'refuse') connection.stop(PROMPT_REFUSED);
  });

  $effect(() => () => {
    connection.close();
  });
</script>

{#snippet toolbar()}
  <RemoteToolbar
    deviceName={device.name}
    {onback}
    docked={onback !== undefined}
    typing={mobile && ready && visible && ask.kind !== 'ask' ? client : undefined}
    {ready}
    policy={rules.notice}
    pasteOff={rules.pasteOff}
    {notice}
    fit={mobile ? () => displayView?.fit() : undefined}
    canFit={displayView?.zoomed() ?? false}
    displays={displayState?.displays}
    switching={displayState?.displayPending ?? false}
    selectDisplay={(id) => connection.tunnel?.selectDisplay?.(id)}
    onkeys={sendKeys}
    onpaste={pasteFromClipboard}
    ondismiss={() => (notice = undefined)}
  />
{/snippet}

<!-- On a touch screen (Back is given) the bar is under the display, where the thumbs and the keyboard are. -->
<section class="session" hidden={!visible} aria-label="{protocol} session on {device.name}">
  {#if onback === undefined}{@render toolbar()}{/if}
  <div class="stage">
    <!-- The keyboard listens here only; menus, panels and modals are outside it (D-8). -->
    <!-- Focusable because the remote desktop takes the keyboard as a text field does (D-8). -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      class="pane"
      class:touch={mobile && ready}
      role="application"
      aria-label="Remote display of {device.name}"
      tabindex="0"
      onpaste={paste}
      {@attach attachPane}
    >
      {#if ready && client !== undefined}
        <RemoteDisplay bind:this={displayView} {client} pane={paneElement} paneSize={paneSize ?? remote} fitKey={displayState?.displays} {remote} {scale} {mobile} interactive={visible && ask.kind !== 'ask' && displayState?.displayPending !== true} />
      {/if}
    </div>
    {#if !ready}
      <ConnectionPanel progress={progressOf(stage)} deviceName={device.name}>
        {#snippet end(underSteps)}
          {#if stage.kind === 'ending' || stage.kind === 'ended'}
            {@const changed = stage.kind === 'ended' && stage.certificateChanged}
            <EndedPanel
              bind:this={ended}
              ending={stage.ending}
              settled={stage.kind === 'ended'}
              certificateChanged={changed}
              onreconnect={changed ? checkCertificate : reconnect}
              {underSteps}
            />
          {/if}
        {/snippet}
      </ConnectionPanel>
    {/if}
  </div>
  {#if onback !== undefined}{@render toolbar()}{/if}
</section>

<!-- One prompt per `required`: a newer one while a save is awaited gets its own prompt, and answers only itself. -->
{#each prompts as prompted (prompted.required)}
  <CredentialPrompt
    {device}
    {protocol}
    fields={prompted.fields}
    client={prompted.client}
    {api}
    onsent={() => (answered = prompted.required)}
    oncancel={() => connection.stop(PROMPT_CANCELLED)}
  />
{/each}

{#if certificateOpen}
  <CertificateModal {device} {api} onclose={() => (dismissed = stage)} onaccepted={reconnect} returnTo={() => ended?.actionButton()} />
{/if}

<style>
  .session {
    display: flex;
    font-family: var(--font-ui);
    flex-direction: column;
    block-size: 100%;
    min-block-size: 0;
    min-inline-size: 0;
    background: var(--surface-base);
    color: var(--text);
  }

  .session[hidden] {
    display: none;
  }

  .stage {
    position: relative;
    flex: 1;
    min-block-size: 0;
  }

  /* The pane's size comes from the layout only, never from the display inside it, so a resize cannot loop (D-5). */
  .pane {
    position: absolute;
    inset: 0;
    overflow: hidden;
    outline: none;
  }

  .pane.touch { touch-action: none; }

  /* Drawn after the display, so a display that fills the pane cannot cover the ring (UI-009, UI-012). */
  .pane:focus-visible::after {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    box-shadow: inset 0 0 0 var(--border-focus) var(--accent);
  }
</style>
