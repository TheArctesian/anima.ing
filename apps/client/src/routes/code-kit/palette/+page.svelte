<script lang="ts">
  import '$lib/code/kit/code.css';
  import CommandPalette from '$lib/code/palette/CommandPalette.svelte';
  import KeyTable from '$lib/code/palette/keys/KeyTable.svelte';
  import type { PaletteAction, PaletteHost } from '$lib/code/palette/adapter';
  import FormButton from '$lib/forms/FormButton.svelte';
  import Segmented from '$lib/forms/segmented/Segmented.svelte';
  import PageHeader from '$lib/page/PageHeader.svelte';
  import Section from '$lib/page/Section.svelte';
  import KitClashes from './KitClashes.svelte';
  import { KIT_CHATS, KIT_NOW } from './chats';

  // The page of stage 6 (TC6, first half): the command palette with a fixed list of chats, its key table and the clashes.
  let open = $state(false);
  let chats = $state<'ready' | 'loading' | 'failed' | 'none'>('ready');
  let chat = $state<'none' | 'open'>('none');
  let platform = $state<'mac' | 'other'>('mac');
  let ran = $state('Nothing yet');

  const say = (action: PaletteAction) => () => {
    ran = action;
  };

  const host: PaletteHost = $derived({
    chats: chats === 'ready' ? KIT_CHATS : [],
    chatsState: chats === 'none' ? 'ready' : chats,
    recent: chats === 'ready' ? ['c2', 'c5', 'c1', 'c7'] : [],
    ...(chat === 'open' ? { openChat: 'c2' } : {}),
    runs: { 'settings.open': say('settings.open'), 'keys.open': say('keys.open'), 'usage.open': say('usage.open') },
    openChatById: (id: string) => {
      ran = `open chat ${id}`;
    },
  });
</script>

<div class="kit">
  <PageHeader title="Command palette" detail="T3 Code's palette and keys in Anima's skin" eyebrow={{ text: 'Stage 6', icon: 'carbon:code' }} />
  <div class="settings">
    <Segmented label="Chats" size="sm" bind:value={chats} options={[{ value: 'ready', label: 'Ready' }, { value: 'loading', label: 'Loading' }, { value: 'failed', label: 'Failed' }, { value: 'none', label: 'None' }]} />
    <Segmented label="Open chat" size="sm" bind:value={chat} options={[{ value: 'none', label: 'None' }, { value: 'open', label: 'One open' }]} />
    <Segmented label="Keys of" size="sm" bind:value={platform} options={[{ value: 'mac', label: 'Mac' }, { value: 'other', label: 'Windows and Linux' }]} />
  </div>
  <Section title="Palette" icon="carbon:search" lede="⌘K or Ctrl+K opens it anywhere on this page. Three actions run here; the rest wait for their stage.">
    <div class="open" data-kit="palette-open">
      <FormButton tone="secondary" onclick={() => (open = true)}>Open the command palette</FormButton>
      <p role="status">Last action: {ran}</p>
    </div>
  </Section>
  <Section title="Keyboard shortcuts" icon="carbon:list">
    <div data-kit="key-table"><KeyTable mac={platform === 'mac'} /></div>
  </Section>
  <Section title="Clashes with keys Anima already takes" icon="carbon:warning-alt" lede="Listed for a ruling; nothing here overrides a key.">
    <div data-kit="clashes"><KitClashes /></div>
  </Section>
</div>

<CommandPalette {host} bind:open now={KIT_NOW} />

<style>
  .kit {
    display: grid;
    gap: var(--space-10);
    max-inline-size: var(--content-page);
    padding-block-end: var(--space-16);
  }

  .settings {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-4);
  }

  .open {
    display: flex;
    align-items: center;
    gap: var(--space-4);
  }

  .open p {
    margin: 0;
    color: var(--text-secondary);
  }
</style>
