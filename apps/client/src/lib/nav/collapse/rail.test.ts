// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, expect, it, vi } from 'vitest';
import { DESKTOP_MAC } from '$lib/platform/answers.fixture';
import Nav from '../Nav.svelte';

vi.mock('$lib/platform/platform', () => ({ capabilities: () => DESKTOP_MAC }));
vi.mock('$lib/hub/context', () => ({
  useHub: () => ({
    address: { origin: 'https://hub.example' },
    status: { current: { connection: 'connected', lastContactAt: null } },
    request: () => new Promise(() => undefined),
    subscribe: () => () => undefined,
  }),
}));
vi.mock('motion', async (importOriginal) => ({
  ...(await importOriginal<typeof import('motion')>()),
  animate: (await import('$lib/motion/engine.fixture')).fakeRun,
}));

let app: ReturnType<typeof mount> | undefined;

afterEach(async () => {
  if (app !== undefined) await unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

function bar(pathname: string, collapsed: boolean, ontogglecollapse = vi.fn()) {
  app = mount(Nav, {
    target: document.body,
    props: { pathname, now: new Date('2026-11-22T12:00:00Z'), iconOnly: collapsed, collapsed, canCollapse: true, ontogglecollapse },
  });
  flushSync();
  const mark = document.querySelector('.brand button');
  return { top: document.querySelector('.bar button')?.getAttribute('aria-label') ?? null, mark: mark?.getAttribute('aria-label') ?? null, title: mark?.getAttribute('title') ?? null };
}

it('NAV-006: collapsed, the top row holds no toggle and the mark opens the bar, with its name and tooltip', async () => {
  const toggle = vi.fn();
  expect(bar('/devices', true, toggle)).toEqual({ top: null, mark: 'Expand sidebar', title: 'Expand sidebar' });
  document.querySelector<HTMLButtonElement>('.brand button')?.click();
  expect(toggle).toHaveBeenCalledOnce();
  await unmount(app as ReturnType<typeof mount>);
  app = undefined;
  expect(bar('/code', true)).toEqual({ top: null, mark: 'Expand sidebar', title: 'Expand sidebar' });
});

it('NAV-006: open, the toggle sits in the top row and the mark is the logo and the name', () => {
  expect(bar('/devices', false)).toEqual({ top: 'Collapse sidebar', mark: null, title: null });
  expect(document.querySelector('.brand .words')?.textContent).toBe('Anima.ing');
});
