import { describe, expect, it } from 'vitest';
import { addressOf } from './address';
import { FREE_VIEWPORT, MAX_SIDE, MIN_SIDE, preset, resized, rotated } from './viewport';
import { moreMenu } from './words';

describe('the address that the person typed', () => {
  it('opens http and https as they are', () => {
    expect(addressOf(' https://example.com/a?b=1 ')).toEqual({ ok: true, url: 'https://example.com/a?b=1' });
    expect(addressOf('http://localhost:5173')).toEqual({ ok: true, url: 'http://localhost:5173/' });
  });

  it('adds https to a host, and http to a host of this computer', () => {
    expect(addressOf('example.com/docs')).toEqual({ ok: true, url: 'https://example.com/docs' });
    expect(addressOf('localhost:5173/sign-in')).toEqual({ ok: true, url: 'http://localhost:5173/sign-in' });
    expect(addressOf('127.0.0.1:8080')).toEqual({ ok: true, url: 'http://127.0.0.1:8080/' });
    expect(addressOf('192.168.1.20')).toEqual({ ok: true, url: 'http://192.168.1.20/' });
  });

  it('refuses each other scheme, with words', () => {
    for (const typed of ['file:///etc/hosts', 'javascript://x%0aalert(1)', 'ftp://example.com', 'chrome://settings', 'data://text/html,x']) {
      expect(addressOf(typed)).toEqual({ ok: false, title: 'Could not open this address', detail: 'Enter an http or https URL.' });
    }
  });

  it('refuses text that is not an address', () => {
    expect(addressOf('').ok).toBe(false);
    expect(addressOf('how to centre a div').ok).toBe(false);
    expect(addressOf('readme').ok).toBe(false);
  });
});

describe('the size of the page', () => {
  it('takes the size of a device, and is free again with no device', () => {
    const phone = preset(FREE_VIEWPORT, 'pixel-7');
    expect(phone).toEqual({ width: 412, height: 915, preset: 'pixel-7', locked: false });
    expect(preset(phone, undefined)).toEqual({ width: 412, height: 915, locked: false });
  });

  it('is free when a side is typed, and keeps a locked shape', () => {
    expect(resized(preset(FREE_VIEWPORT, 'iphone-se'), 'width', 500)).toEqual({ width: 500, height: 667, locked: false });
    expect(resized({ width: 400, height: 800, locked: true }, 'width', 200)).toEqual({ width: 200, height: 400, locked: true });
    expect(resized({ width: 400, height: 800, locked: true }, 'height', 400).width).toBe(200);
  });

  it('keeps a side in its range and ignores what is not a number', () => {
    expect(resized(FREE_VIEWPORT, 'width', 3).width).toBe(MIN_SIDE);
    expect(resized(FREE_VIEWPORT, 'height', 1e9).height).toBe(MAX_SIDE);
    expect(resized(FREE_VIEWPORT, 'width', Number.NaN)).toBe(FREE_VIEWPORT);
  });

  it('turns a quarter and keeps the device', () => {
    expect(rotated(preset(FREE_VIEWPORT, 'iphone-se'))).toMatchObject({ width: 667, height: 375, preset: 'iphone-se' });
  });
});

describe('the more menu of the browser', () => {
  const labels = (state: Parameters<typeof moreMenu>[0]): string[] => moreMenu(state).flatMap((entry) => (entry.kind === 'line' ? [] : [entry.label]));

  it('lists the choices in the order of T3 Code', () => {
    expect(labels({ deviceToolbar: false, appearance: 'system', zoom: 100 })).toEqual([
      'Find on page', 'Hard reload', 'Show device toolbar', 'Appearance', 'Zoom out', 'Zoom in', 'Reset zoom', 'Clear cookies', 'Clear cache',
    ]);
  });

  it('shows what the place of the page has, and the states that are on', () => {
    const entries = moreMenu({ deviceToolbar: true, appearance: 'dark', zoom: 125, devTools: true, profile: 'Work', findKey: '⌘F' });
    expect(labels({ deviceToolbar: true, appearance: 'dark', zoom: 125, devTools: true, profile: 'Work' })).toEqual(expect.arrayContaining(['Open DevTools', 'Hide device toolbar', 'Profile: Work']));
    const choice = (id: string) => entries.find((entry) => entry.kind === 'choice' && entry.id === id);
    expect(choice('find')).toMatchObject({ shortcut: '⌘F' });
    expect(choice('zoom_reset')).toMatchObject({ detail: '125%', disabled: false });
    expect(choice('appearance')).toMatchObject({ items: [{ id: 'appearance:system' }, { id: 'appearance:light' }, { id: 'appearance:dark', detail: 'On' }] });
  });
});
