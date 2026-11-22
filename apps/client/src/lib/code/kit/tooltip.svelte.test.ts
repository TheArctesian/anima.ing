// @vitest-environment jsdom
import { createRawSnippet, flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it } from 'vitest';
import Tooltip from './Tooltip.svelte';

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

/** The words a screen reader takes as the description of the control in a tip of `text`. */
function description(text: string, control: string): string {
  document.body.innerHTML = '<p id="own">The runner cannot attach files yet.</p><div></div>';
  const target = document.querySelector('div') ?? expect.fail('no target');
  const children = createRawSnippet(() => ({ render: () => control }));
  app = mount(Tooltip, { target, props: { text, children } });
  flushSync();
  const button = target.querySelector('button') ?? expect.fail('no button');
  return (button.getAttribute('aria-describedby') ?? '')
    .split(' ')
    .filter((token) => token !== '')
    .map((token) => document.getElementById(token)?.textContent ?? '')
    .join(' | ');
}

it('09 finding 8: the tip words describe the control itself', () => {
  expect(description('Snooze until later', '<button aria-label="Snooze">z</button>')).toBe('Snooze until later');
});

it('keeps a description the control already has, and adds its own words after it', () => {
  expect(description('Attach files', '<button aria-label="Attach" aria-describedby="own">a</button>')).toBe('The runner cannot attach files yet. | Attach files');
});

it('says nothing twice: no words that repeat the name or the description already there', () => {
  expect(description('Snooze', '<button aria-label="Snooze">z</button>')).toBe('');
  expect(description('The runner cannot attach files yet.', '<button aria-label="Attach" aria-describedby="own">a</button>')).toBe('The runner cannot attach files yet.');
});
