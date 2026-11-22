<script lang="ts">
  import CodeIcon from '../../icons/CodeIcon.svelte';
  import IconButton from '../../kit/IconButton.svelte';
  import { MAX_RECENT, START_WORDS } from './words';

  /** An address that the person opened before. */
  export interface RecentPage {
    readonly url: string;
    readonly title?: string | undefined;
  }

  /** A server of this thread that listens now. */
  export interface LocalServer {
    readonly url: string;
    /** What runs it: "vite". */
    readonly process?: string | undefined;
  }

  interface Props {
    recent?: readonly RecentPage[];
    servers?: readonly LocalServer[];
    onopen: (url: string) => void;
    /** Takes an address out of the history. */
    onforget?: ((url: string) => void) | undefined;
  }

  const { recent = [], servers = [], onopen, onforget }: Props = $props();
  const shown = $derived(recent.slice(0, MAX_RECENT));
</script>

<!-- The start page of a browser tab (report 05, Section 6.4): nothing known, or the addresses used before and the local servers. -->
{#if shown.length === 0 && servers.length === 0}
  <div class="nothing">
    <CodeIcon name="globe" />
    <h3>{START_WORDS.title}</h3>
    <p>{START_WORDS.detail}</p>
  </div>
{:else}
  <div class="start">
    <div class="column">
      {#if shown.length > 0}
        <section aria-labelledby="browser-recent">
          <h3 id="browser-recent"><CodeIcon name="history" />{START_WORDS.recent}</h3>
          <ul>
            {#each shown as page (page.url)}
              <li>
                <button type="button" class="card" onclick={() => onopen(page.url)}>
                  <span class="name">{page.title ?? page.url}</span>
                  {#if page.title !== undefined}<span class="url">{page.url}</span>{/if}
                </button>
                {#if onforget !== undefined}<IconButton label="Remove {page.url} from history" icon="x" onclick={() => onforget(page.url)} />{/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
      {#if servers.length > 0}
        <section aria-labelledby="browser-servers">
          <h3 id="browser-servers"><CodeIcon name="radio-tower" />{START_WORDS.servers}</h3>
          <ul>
            {#each servers as server (server.url)}
              <li>
                <button type="button" class="card" onclick={() => onopen(server.url)}>
                  <span class="name">{server.url}</span>
                  <span class="url">{server.process === undefined ? START_WORDS.listening : `${server.process} · ${START_WORDS.listening}`}</span>
                </button>
              </li>
            {/each}
          </ul>
          <p class="hint">{START_WORDS.serversHint}</p>
        </section>
      {/if}
    </div>
  </div>
{/if}

<style>
  .nothing {
    display: flex;
    flex: 1;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--space-2);
    padding: var(--space-6);
    color: var(--code-quiet-text);
    font-size: var(--code-text-small);
    text-align: center;
  }

  .nothing h3 {
    margin: 0;
    color: var(--text);
    font-size: var(--code-text-body);
    font-weight: var(--weight-medium);
  }

  .nothing p {
    max-inline-size: calc(var(--space-16) * 6);
    margin: 0;
    line-height: var(--leading-relaxed);
  }

  .start {
    flex: 1;
    min-block-size: 0;
    padding: var(--space-8) var(--space-5);
    overflow-y: auto;
  }

  .column {
    display: grid;
    gap: var(--space-6);
    max-inline-size: calc(var(--space-16) * 9);
    margin-inline: auto;
  }

  h3 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0 0 var(--space-2);
    color: var(--text);
    font-size: var(--code-text-small);
    font-weight: var(--weight-medium);
  }

  ul {
    display: grid;
    gap: var(--space-1);
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    border: var(--border-thin) solid var(--code-line);
    border-radius: var(--radius-md);
    padding-inline-end: var(--space-1);
  }

  li:hover {
    background: var(--surface-hover);
  }

  .card {
    display: grid;
    flex: 1;
    gap: calc(var(--space-1) / 2);
    min-inline-size: 0;
    padding: var(--space-2) var(--space-3);
    border: 0;
    border-radius: var(--radius-md);
    background: none;
    color: var(--text);
    font: inherit;
    font-size: var(--code-text-small);
    text-align: start;
  }

  .card:focus-visible {
    outline: var(--border-focus) solid var(--focus-ring);
    outline-offset: calc(var(--border-focus) * -1);
  }

  .name,
  .url {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .url,
  .hint {
    color: var(--code-quiet-text);
    font-size: var(--code-text-meta);
  }

  .hint {
    margin: var(--space-2) 0 0;
  }
</style>
