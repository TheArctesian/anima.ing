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
  import KitWebKeys from './KitWebKeys.svelte';
  import { KIT_CHATS, KIT_NOW } from './chats';

  let open = $state(false);
  let chats = $state<'ready' | 'loading' | 'failed' | 'none'>('ready');
  let chat = $state<'none' | 'open'>('none');
  let platform = $state<'mac' | 'other'>('mac');
  let client = $state<'desktop' | 'web'>('desktop');
  const keys = $derived({ mac: platform === 'mac', web: client === 'web' });
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
    <Segmented label="Open chat" size="sm" bind:value={chat} options={[{ value: 'none', label: 'No chat open' }, { value: 'open', label: 'One open' }]} />
    <Segmented label="Keys of" size="sm" bind:value={platform} options={[{ value: 'mac', label: 'Mac' }, { value: 'other', label: 'Windows and Linux' }]} />
    <Segmented label="Client" size="sm" bind:value={client} options={[{ value: 'desktop', label: 'Desktop app' }, { value: 'web', label: 'Web' }]} />
  </div>
  <Section title="Palette" icon="carbon:search" lede="Its key for the platform and client chosen above opens it anywhere on this page. Three actions run here; the rest wait for their stage.">
    <div class="open" data-kit="palette-open">
      <FormButton tone="secondary" onclick={() => (open = true)}>Open the command palette</FormButton>
      <p role="status">Last action: {ran}</p>
    </div>
  </Section>
  <Section title="Keyboard shortcuts" icon="carbon:list">
    <div data-kit="key-table"><KeyTable {keys} /></div>
  </Section>
  <Section title="Keys of the web client" icon="carbon:earth" lede="Each T3 Code key a browser keeps, and the key the web client uses in its place (ruling: Remap for web).">
    <div data-kit="web-keys"><KitWebKeys /></div>
  </Section>
  <Section title="Clashes with keys Anima already takes" icon="carbon:warning-alt" lede="The clashes left after the web keys, each with the ruling that keeps it.">
    <div data-kit="clashes"><KitClashes /></div>
  </Section>
</div>

<CommandPalette {host} bind:open now={KIT_NOW} {keys} />

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
