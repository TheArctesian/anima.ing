<script lang="ts">
  import { clashes, type KeyPlace } from '$lib/code/palette/keys/clashes';
  import { chordText, parseChord } from '$lib/code/palette/keys/keymap';
  import { COMMAND_WORDS, whenWords } from '$lib/code/palette/keys/words';

  const PLACES: Readonly<Record<KeyPlace, string>> = {
    'threads-page': 'The threads page',
    'session-terminal': 'A session terminal',
    'session-tab': 'A session tab',
    'mac-app-menu': 'The menu of the Mac app',
    browser: 'The browser',
    windows: 'Windows',
  };

  const rows = clashes().map((clash) => {
    const chord = parseChord(clash.key);
    const mac = clash.platform === 'mac';
    return {
      key: `${clash.client}|${clash.platform}|${clash.key}|${clash.command}|${clash.taken.place}`,
      keys: chord === null ? clash.key : chordText(chord, mac),
      platform: `${clash.client === 'web' ? 'Web' : 'Desktop app'}, ${mac ? 'Mac' : 'Windows and Linux'}`,
      code: `${COMMAND_WORDS[clash.command]?.label ?? clash.command} (${whenWords(clash.when)})`,
      taken: `${PLACES[clash.taken.place]}: ${clash.taken.does}`,
      kept: clash.taken.kept ?? 'Not ruled yet',
    };
  });
</script>

<!-- Each key of T3 Code's table that a key Anima, its window or the browser already takes would meet (clashes.ts). -->
<table>
  <thead>
    <tr><th scope="col">Keys</th><th scope="col">Client</th><th scope="col">In Code</th><th scope="col">Already</th><th scope="col">Kept because</th></tr>
  </thead>
  <tbody>
    {#each rows as row (row.key)}
      <tr><th scope="row">{row.keys}</th><td>{row.platform}</td><td>{row.code}</td><td>{row.taken}</td><td>{row.kept}</td></tr>
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

  tbody th {
    white-space: nowrap;
  }
</style>
