import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';

/** jsdom does not load a component's styles, so this reads the rule from the source. */
function rule(file: string, selector: string): string {
  const text = readFileSync(new URL(file, import.meta.url), 'utf8');
  // The last rule of the selector: a grouped rule above it may end with the same selector.
  const start = text.lastIndexOf(`  ${selector} {`);
  if (start < 0) throw new Error(`no rule ${selector} in ${file}`);
  return text.slice(start, text.indexOf('}', start));
}

it('MOT-007: the scroll clearance is a constant: the 66 % stop plus the header, never the live height', () => {
  expect(rule('../../devices/screen/DeviceScreen.svelte', '.clearance')).toContain(
    'calc(var(--logs-two-thirds) + var(--logs-header-size))',
  );
});
