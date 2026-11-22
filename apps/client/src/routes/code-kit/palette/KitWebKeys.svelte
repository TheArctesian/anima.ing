<script lang="ts">
  import { chordText, parseChord } from '$lib/code/palette/keys/keymap';
  import { WEB_KEYS, webKeyOn } from '$lib/code/palette/keys/web';
  import { COMMAND_WORDS } from '$lib/code/palette/keys/words';

  const PLATFORMS = { mac: 'Mac', other: 'Windows and Linux' } as const;

  /** A key on one platform, written for it; `web` keys take ⌃⇧ on the Mac and Alt+Shift elsewhere. */
  function on(key: string, mac: boolean, web: boolean): string {
    const chord = parseChord(web ? webKeyOn(key, mac) : key);
    return chord === null ? key : chordText(chord, mac);
  }

  /** A key on the Mac and elsewhere, "⌘N · Ctrl+N", or on the one platform the row is for. */
  function written(key: string | null, only: 'mac' | 'other' | undefined, web: boolean): string {
    if (key === null) return 'None';
    if (only !== undefined) return on(key, only === 'mac', web);
    const mac = on(key, true, web);
    const other = on(key, false, web);
    return mac === other ? mac : `${mac} · ${other}`;
  }

  const rows = WEB_KEYS.map((web) => ({
    id: `${web.from}|${web.command}|${web.only ?? ''}`,
    command: COMMAND_WORDS[web.command]?.label ?? web.command,
    t3: written(web.from, web.only, false),
    key: written(web.key, web.only, true),
    platform: web.only === undefined ? 'Both' : PLATFORMS[web.only],
    why: web.why,
  }));
</script>

<!-- The web keys (keys/web.ts): each T3 Code key a browser keeps, the key in its place, and what the browser does with it. -->
<table>
  <thead>
    <tr><th scope="col">Command</th><th scope="col">T3 Code</th><th scope="col">Web client</th><th scope="col">Platform</th><th scope="col">The browser's key</th></tr>
  </thead>
  <tbody>
    {#each rows as row (row.id)}
      <tr><th scope="row">{row.command}</th><td class="keys">{row.t3}</td><td class="keys web">{row.key}</td><td>{row.platform}</td><td>{row.why}</td></tr>
    {/each}
  </tbody>
</table>

<style>
  table {
    inline-size: 100%;
    border-collapse: collapse;
    font-size: var(--text-sm);
  }

  th,
  td {
    padding: var(--space-1) var(--space-2);
    font-weight: var(--weight-regular);
    text-align: start;
    vertical-align: top;
  }

  thead th {
    color: var(--text-secondary);
    font-size: var(--text-xs);
    font-weight: var(--weight-medium);
  }

  tbody tr {
    border-block-start: var(--border-thin) solid var(--border-subtle);
  }

  .keys {
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .web {
    font-weight: var(--weight-medium);
  }
</style>
