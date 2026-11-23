import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { BUILD_FOLDER } from '../synthetic/static-files';

// The build hashes chunk names, so the updater chunks are found by what they hold, not by name (UPD-001).

const IMMUTABLE = join(BUILD_FOLDER, '_app', 'immutable');
const PLUGIN_COMMAND = 'plugin:updater|';

function scriptsUnder(folder: string): string[] {
  return readdirSync(folder, { withFileTypes: true }).flatMap((entry) => {
    const path = join(folder, entry.name);
    if (entry.isDirectory()) return scriptsUnder(path);
    return entry.name.endsWith('.js') ? [path] : [];
  });
}

/** The layout's chunk names only the updater module, not the plugin, so it is not in the list. */
export function updaterChunkPaths(): string[] {
  const scripts = scriptsUnder(IMMUTABLE).map((path) => ({ path, text: readFileSync(path, 'utf8') }));
  const plugin = scripts.filter((script) => script.text.includes(PLUGIN_COMMAND));
  const namesPlugin = (text: string): boolean => plugin.some((found) => text.includes(found.path.slice(found.path.lastIndexOf('/') + 1)));
  const holding = [...plugin, ...scripts.filter((script) => !plugin.includes(script) && namesPlugin(script.text))];
  return holding.map((script) => script.path.slice(BUILD_FOLDER.length - 1));
}
