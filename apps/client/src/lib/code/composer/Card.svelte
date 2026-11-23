<script lang="ts">
  import { onDestroy, tick, untrack } from 'svelte';
  import { isMac } from '$lib/platform/mac';
  import AttachButton from './attachments/AttachButton.svelte';
  import Attachments from './attachments/Attachments.svelte';
  import Preview from './attachments/Preview.svelte';
  import { LIMITS, pasteDecision } from './attachments/attach';
  import { dropZone } from './attachments/drop';
  import { CardState } from './card-state.svelte';
  import { Favourites } from './favourites/favourites.svelte';
  import type { CaretAt } from './draft';
  import Editor from './editor/Editor.svelte';
  import type { ComposerEngine } from './engine';
  import MentionMenu from './mentions/MentionMenu.svelte';
  import { chipReader } from './mentions/chips';
  import { insertionOf } from './mentions/insertion';
  import type { MentionEntry } from './mentions/sources';
  import { afterCaretMove, afterEdit, afterEscape, NO_TRIGGER, triggerAt } from './mentions/trigger';
  import ModelPicker from './picker/ModelPicker.svelte';
  import PermissionsChip from './picker/popup/PermissionsChip.svelte';
  import SendButton from './primary/SendButton.svelte';
  import StopButton from './primary/StopButton.svelte';
  import { enterSends, showsSend, showsStop } from './primary/send';
  import QueuedMessages from './queue/QueuedMessages.svelte';
  import { labelled, queueKeys, sendsFirst } from './queue/queue';
  import MicButton from './voice/MicButton.svelte';
  import './composer.css';
  import './mentions/mentions.css';

  interface Props {
    engine: ComposerEngine;
    /** The key under which the unsent text of this chat is kept. */
    draft?: string | undefined;
    /** The chat's title, for the editor's name: "Message Fix the gate". */
    title?: string | undefined;
    /** The text to start from, for the specimen and the gallery. */
    start?: string;
    /** The strip under the card of a chat that is not started, with the permissions at its end (03 B.6.5; T3 shows them there only on a new thread). */
    contextStrip?: boolean;
  }

  /*
   * The composer's card (03 B.1): the queued messages above it, then the editor with what is attached, and the
   * footer with the paperclip, the model, the microphone and the round send. The card is opaque and shows a focus
   * ring while the editor has focus: T3's is glass with no ring (R-354, D7 and D9).
   */
  const { engine, draft, title, start, contextStrip = false }: Props = $props();
  const menuId = $props.id();
  // The chat column mounts one composer for each chat (`{#key}` in Composer.svelte), so the key is read once.
  // svelte-ignore state_referenced_locally
  const card = new CardState(() => engine, draft, isMac(navigator.userAgent) ? '⌘' : 'Ctrl');
  const favourites = new Favourites(localStorage);
  // svelte-ignore state_referenced_locally
  if (start !== undefined) card.prompt = start;
  // One reader while the skills are the same list: a new one makes the editor draw the whole prompt again.
  const skills = $derived(engine.sources.skills);
  const chips = $derived(chipReader({ skills }));
  let editor = $state<Editor>();
  let menu = $state<MentionMenu>();
  let activeOption = $state<string>();
  let surface = $state<HTMLElement>();
  let root = $state<HTMLElement>();
  let over = $state(false);
  let watch = $state(NO_TRIGGER);
  let caret = $state<DOMRect | null>(null);
  let seen = '';
  let caretAt = 0;
  let allQueued = $state(false);
  let picking = $state(false);
  let previewed = $state<string | null>(null);
  // No list exists for pull requests yet, so `#` opens nothing.
  const trigger = $derived(watch.open !== null && watch.open.kind !== 'pull-request' ? watch.open : null);
  const images = $derived(card.attachments.filter((attachment) => attachment.kind === 'image' && attachment.state.state === 'ready'));

  $effect(() => {
    card.turned(engine.phase === 'running');
  });
  $effect(() => {
    card.keep();
  });
  $effect(() => {
    const open = card.editing;
    if (open !== null && !engine.queued.some((queued) => queued.id === open)) untrack(() => card.orphaned());
  });
  $effect(() => {
    const now = engine.choice;
    untrack(() => card.settled(now));
  });
  onDestroy(() => card.dispose());

  /* The editor tells this from inside its own effect: nothing read here may become a dependency of that effect. */
  function onCaret(at: CaretAt): void {
    untrack(() => {
      const found = at.rect === null ? null : triggerAt(at.prompt, at.caret);
      watch = at.prompt === seen ? afterCaretMove(watch, found) : afterEdit(watch, found);
      seen = at.prompt;
      caretAt = at.caret;
      caret = at.rect;
    });
  }

  function choose(choice: MentionEntry): void {
    if (trigger === null) return;
    const insertion = insertionOf(trigger, choice, card.prompt);
    watch = NO_TRIGGER;
    if (insertion === null) return;
    editor?.replace(insertion.start, insertion.end, insertion.text);
    if (choice.kind === 'command' && choice.command.name === 'model') picking = true;
  }

  /** The two keys of the queue (03 3.5): the first message goes now; the last opens in the composer. */
  function queueKey(event: KeyboardEvent): boolean {
    if (card.busy || event.isComposing) return false;
    if (sendsFirst({ key: event.key, shift: event.shiftKey, alt: event.altKey, command: event.metaKey || event.ctrlKey })) {
      const first = engine.queued[0];
      if (first !== undefined && !first.saving) void card.queued(first.id, 'send');
      return true;
    }
    // With the caret at the start, also over a prompt that has text: it is set aside (T3 Code `ChatView.tsx:2895-2900`).
    const last = engine.queued.at(-1);
    if (event.key !== 'ArrowUp' || !event.altKey || caretAt !== 0 || card.editing !== null || last === undefined || last.saving) return false;
    allQueued = true;
    card.edit(last.id);
    return true;
  }

  /** The keys of the composer before the editor's own (03 B.12): the open menu first, then the queue, then Enter. */
  function onKey(event: KeyboardEvent): boolean {
    if (trigger !== null && menu?.key(event) === true) return true;
    if (queueKey(event)) return true;
    // ⌘⇧M, or Ctrl+Shift+M: the model picker (T3 Code `keybindings.ts:115`).
    if (event.code === 'KeyM' && event.shiftKey && (event.metaKey || event.ctrlKey) && !event.altKey) {
      if (engine.pick.can) picking = !picking;
      return true;
    }
    if (!enterSends({ key: event.key, shift: event.shiftKey, alt: event.altKey, composing: event.isComposing })) return false;
    void card.submit(event.metaKey || event.ctrlKey);
    return true;
  }

  /** A long paste (03 "Paste decision"): a text file where the engine takes files, else in the text while it fits. */
  function longPaste(text: string): void {
    const end = card.prompt.length;
    const room = engine.attach.can && card.attachments.length < LIMITS.count;
    const names = card.attachments.map((attachment) => attachment.name);
    const decision = pasteDecision({ files: [], folders: 0, plainText: text, asText: false, chips: false }, { promptLength: end, selection: { start: end, end }, names, room });
    if (decision.act === 'fold') card.add([new File([text], decision.name, { type: decision.mime })]);
    else if (decision.act === 'refuse') card.problem = `${decision.title}. ${decision.description}`;
    else editor?.insert(text);
  }

  /**
   * A control that goes away when it is pressed (Send, Stop, Remove): if the focus was in the composer and is nowhere
   * after, it goes to the editor. A press by a pointer never had the focus, so no keyboard comes up on a phone.
   */
  function pressed(act: () => unknown): void {
    const inside = root?.contains(document.activeElement) === true;
    void act();
    if (!inside) return;
    void tick().then(() => {
      if (document.activeElement === null || document.activeElement === document.body) editor?.focus();
    });
  }

  function held(event: KeyboardEvent): void {
    card.alternate = event.metaKey || event.ctrlKey;
  }
</script>

<svelte:window onkeydown={held} onkeyup={held} onblur={() => (card.alternate = false)} />

<div class="composer" bind:this={root}>
  <QueuedMessages
    queue={engine.queued}
    waits={engine.queueWaits ?? null}
    keys={queueKeys(card.modifier === '⌘')}
    editing={card.editing}
    running={card.running}
    busy={card.busy}
    bind:all={allQueued}
    onSendNow={(id) => pressed(() => card.queued(id, 'send'))}
    onEdit={(id) => {
      card.edit(id);
      editor?.focus();
    }}
    onRemove={(id) => pressed(() => card.queued(id, 'remove'))}
    onCancel={() => {
      card.cancelEdit();
      editor?.focus();
    }}
  >
    {#snippet body(prompt)}<p class="queued">{labelled(prompt, chips(prompt))}</p>{/snippet}
  </QueuedMessages>
  {#if trigger !== null}
    <MentionMenu bind:this={menu} id={menuId} {trigger} body={surface} anchor={caret} sources={engine.sources} bind:activeOption onChoose={choose} onClose={() => (watch = afterEscape(trigger))} />
  {/if}
  <form
    class="card"
    class:over
    aria-label="Composer"
    bind:this={surface}
    {@attach dropZone({
      onOver: (now) => (over = now && engine.attach.can),
      onFiles: (files, folders) => {
        if (engine.attach.can) card.add(files, folders.length);
        else card.problem = engine.attach.reason;
      },
    })}
    onsubmit={(event) => {
      event.preventDefault();
    }}
  >
    <div class="body">
      <Attachments
        attachments={card.attachments}
        onRemove={(id) => pressed(() => card.remove(id))}
        onOpen={(id) => (previewed = id)} />
      <Editor
        bind:this={editor}
        bind:value={card.prompt}
        label={title === undefined ? 'Message' : `Message ${title}`}
        placeholder="Ask anything. @ files, $ skills, / commands"
        {chips}
        {onCaret}
        {onKey}
        controls={trigger === null ? undefined : menuId}
        activeDescendant={trigger === null ? undefined : activeOption}
        onFiles={(files, how, folders) => {
          // A drop goes on to the card's own drop zone, which takes it one time.
          if (how === 'drop') return true;
          if (engine.attach.can) card.add(files, folders);
          else card.problem = engine.attach.reason;
          return engine.attach.can;
        }}
        onLongPaste={longPaste}
      />
    </div>
    {#if card.problem !== null}<p class="problem" role="alert">{card.problem}</p>{/if}
    <div class="footer">
      <AttachButton can={engine.attach} onFiles={(files) => card.add(files)} />
      <div class="actions">
        <ModelPicker catalogue={favourites.apply(engine.catalogue)} onFavourite={favourites.set} value={card.shownChoice(engine.choice)} onChange={(next) => void card.choose(next)} can={engine.pick} onAsk={engine.askModels} shortcut={card.modifier === '⌘' ? '⌘⇧M' : 'Ctrl+Shift+M'} align="end" bind:open={picking} />
        <!-- T3 mounts the microphone only where dictation works (`ChatComposer.tsx:3897`); ours has none yet. -->
        {#if engine.dictate.can}<MicButton can={engine.dictate} onStart={() => undefined} />{/if}
        {#if showsSend(card.turn)}<SendButton face={card.face} onPress={(alternate) => pressed(() => card.submit(alternate))} />{/if}
        {#if showsStop(card.turn, card.stop)}<StopButton pending={card.stop === 'pending'} onStop={() => pressed(() => card.interrupt())} />{/if}
      </div>
    </div>
  </form>
  {#if contextStrip && engine.pick.can}
    {@const shown = card.shownChoice(engine.choice)}
    <div class="context-strip" data-slot="composer-context-strip">
      <PermissionsChip modes={engine.catalogue.modes} value={shown.mode} onChange={(mode) => void card.choose({ ...shown, mode })} />
    </div>
  {/if}
</div>
<Preview {images} openId={previewed} onOpen={(id) => (previewed = id)} onClose={() => (previewed = null)} />

<style>
  .composer {
    display: flex;
    position: relative;
    flex-direction: column;
    inline-size: 100%;
    font-family: var(--font-ui);
  }

  /* Above the mention menu, whose foot is tucked under the card's top edge (03 "Popup design"). */
  .card {
    position: relative;
    border: var(--border-thin) solid var(--code-composer-line);
    border-radius: var(--code-composer-radius);
    background: var(--code-composer-surface);
    transition: background-color var(--duration-base) var(--ease-enter);
  }

  .card:has(:global([role='textbox']:focus)) {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-thin) * -1);
  }

  /* T3's context strip (03 A.8): 8 px over it and at its sides, 4 px between its parts, 12 px text; bare, under the card. */
  .context-strip {
    display: flex;
    gap: var(--space-1);
    align-items: center;
    inline-size: 100%;
    padding: var(--space-2) var(--space-2) 0;
    font-size: var(--code-text-meta);
  }

  /* Files are dragged over it (03 B.2.6): a fill and a line in the accent. */
  .card.over {
    border-color: var(--accent);
    background: var(--accent-tint);
  }

  .body {
    position: relative;
    padding: var(--code-composer-pad-start) var(--code-composer-pad-inline) var(--code-composer-pad-end);
    font-size: var(--code-text-body);
  }

  .problem {
    margin: 0;
    padding: 0 var(--code-composer-pad-inline) var(--space-2);
    color: var(--status-down-text);
    font-size: var(--code-text-meta);
  }

  .footer {
    display: flex;
    align-items: center;
    min-inline-size: 0;
    padding: 0 var(--code-composer-footer-pad) var(--code-composer-footer-pad);
  }

  .actions {
    display: flex;
    flex: 1;
    gap: var(--code-composer-actions-gap);
    align-items: center;
    justify-content: flex-end;
    min-inline-size: 0;
    margin-inline-start: var(--space-2);
  }

  .queued {
    margin: 0;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
</style>
