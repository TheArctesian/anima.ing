import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { HUB_HOST } from '../synthetic/access';
import { inlineScriptHashes } from '../synthetic/inline-scripts';
import { BUILD_FOLDER } from '../synthetic/static-files';

// The policy comes from the K7 contract text, not the synthetic hub's code, so a harness that drifted cannot pass a clean run.

const CONVENTIONS = fileURLToPath(new URL('../../../../contracts/conventions.md', import.meta.url));
const OPENING = 'Static file headers: `Content-Security-Policy: ';
const HASHES = ' <SvelteKit hashes>';
const HOST = '<public host>';

export function builtIndex(): string {
  return readFileSync(`${BUILD_FOLDER}index.html`, 'utf8');
}

function contractTemplate(): string {
  const text = readFileSync(CONVENTIONS, 'utf8');
  const start = text.indexOf(OPENING);
  const end = text.indexOf('`', start + OPENING.length);
  if (start === -1 || end === -1) throw new Error('contracts/conventions.md has no K7 Content-Security-Policy');
  return text.slice(start + OPENING.length, end).replace(/\s+/g, ' ');
}

export function k7Policy(hashes: readonly string[] = inlineScriptHashes(builtIndex())): string {
  const template = contractTemplate();
  if (!template.includes(HASHES) || !template.includes(HOST)) throw new Error('the K7 policy template changed shape');
  return template.replace(HASHES, hashes.map((hash) => ` ${hash}`).join('')).replace(HOST, HUB_HOST);
}
