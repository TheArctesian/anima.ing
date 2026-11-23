// @vitest-environment jsdom
import { createRawSnippet, flushSync, mount, unmount, type ComponentProps } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import BrowserSurface from './BrowserSurface.svelte';
import type { Viewport } from './viewport';

type Props = ComponentProps<typeof BrowserSurface>;
let app: ReturnType<typeof mount> | undefined;

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.replaceChildren();
});

const page = createRawSnippet<[Viewport | undefined]>(() => ({ render: () => '<p data-web-page>the page</p>' }));

function draw(props: Partial<Props> = {}, bar: Partial<Props['toolbar']> = {}): Record<'onback' | 'onforward' | 'onreload' | 'onstop' | 'onopen' | 'onmore' | 'onexternal', ReturnType<typeof vi.fn>> {
  const on = { onback: vi.fn(), onforward: vi.fn(), onreload: vi.fn(), onstop: vi.fn(), onopen: vi.fn(), onmore: vi.fn(), onexternal: vi.fn() };
  const toolbar = { url: 'http://localhost:5173/', more: { deviceToolbar: false, appearance: 'system' as const, zoom: 100 }, ...on, ...bar };
  app = mount(BrowserSurface, { target: document.body, props: { view: { kind: 'page' }, toolbar, start: { onopen: on.onopen }, page, ...props } });
  flushSync();
  return on;
}

const button = (name: string): HTMLButtonElement | null => document.querySelector<HTMLButtonElement>(`button[aria-label="${name}"]`);
const address = (): HTMLInputElement => document.querySelector<HTMLInputElement>('input[name="address"]') ?? document.createElement('input');

function type(text: string): void {
  address().value = text;
  address().dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  document.querySelector('form')?.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
  flushSync();
}

describe('the row of the browser surface', () => {
  it('turns back and forward off with no history, and stops a page that loads', () => {
    const on = draw({}, { canBack: true, loading: true });
    expect(button('Back')?.disabled).toBe(false);
    expect(button('Forward')?.disabled).toBe(true);
    button('Stop')?.click();
    expect(on.onstop).toHaveBeenCalledOnce();
    expect(on.onreload).not.toHaveBeenCalled();
    expect(document.querySelector('[role="progressbar"]')).not.toBeNull();
  });

  it('opens an address that the person typed, with its scheme', () => {
    const on = draw();
    type('localhost:8080/health');
    expect(on.onopen).toHaveBeenCalledWith('http://localhost:8080/health');
  });

  it('refuses an address that is not http or https, says why and keeps what was typed', () => {
    const on = draw();
    type('file:///etc/hosts');
    expect(on.onopen).not.toHaveBeenCalled();
    expect(document.querySelector('[role="alert"]')?.textContent).toBe('Could not open this address. Enter an http or https URL.');
    expect(address().getAttribute('aria-invalid')).toBe('true');
    expect(address().value).toBe('file:///etc/hosts');
  });

  it('puts the address of the page back on Escape', () => {
    draw();
    address().value = 'example';
    address().dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    address().dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    flushSync();
    expect(address().value).toBe('http://localhost:5173/');
  });
});

describe('what a browser tab shows', () => {
  it('says that there is no preview when nothing is known', () => {
    draw({ view: { kind: 'start' } }, { url: '' });
    expect(document.body.textContent).toContain('No preview yet');
    expect(button('Refresh')?.disabled).toBe(true);
    expect(document.querySelector('[data-web-page]')).toBeNull();
  });

  it('lists eight used addresses at most and the local servers, and opens one', () => {
    const recent = Array.from({ length: 10 }, (_, index) => ({ url: `https://example.com/${String(index)}` }));
    const onforget = vi.fn();
    const on = draw({ view: { kind: 'start' }, start: { recent, servers: [{ url: 'http://localhost:5173', process: 'vite' }], onopen: vi.fn(), onforget } });
    expect(document.querySelectorAll('[aria-labelledby="browser-recent"] li')).toHaveLength(8);
    button('Remove https://example.com/0 from history')?.click();
    expect(onforget).toHaveBeenCalledWith('https://example.com/0');
    expect(document.querySelector('[aria-labelledby="browser-servers"] .url')?.textContent).toBe('vite · Listening');
    expect(on.onopen).not.toHaveBeenCalled();
  });

  it('gives the reason and a reload when the site did not answer', () => {
    const on = draw({ view: { kind: 'unreachable', detail: 'ERR_CONNECTION_REFUSED' } });
    expect(document.body.textContent).toContain("This site can't be reached");
    const details = [...document.querySelectorAll('button')].find((each) => each.textContent === 'Details');
    expect(document.querySelector('pre')?.hidden).toBe(true);
    details?.click();
    flushSync();
    expect(document.querySelector('pre')?.hidden).toBe(false);
    expect(details?.textContent).toBe('Hide details');
    [...document.querySelectorAll('button')].find((each) => each.textContent === 'Reload')?.click();
    expect(on.onreload).toHaveBeenCalledOnce();
  });

  it('sizes the page by the device toolbar and closes the toolbar through the menu choice', () => {
    const onviewport = vi.fn();
    const on = draw({ viewport: { width: 375, height: 667, preset: 'iphone-se' }, onviewport });
    expect(document.querySelector<HTMLElement>('.page')?.style.inlineSize).toBe('375px');
    button('Rotate viewport')?.click();
    expect(onviewport).toHaveBeenCalledWith({ width: 667, height: 375, preset: 'iphone-se' });
    const width = document.querySelector<HTMLInputElement>('input[aria-label="Viewport width"]');
    if (width !== null) width.value = '10';
    width?.dispatchEvent(new Event('change', { bubbles: true }));
    expect(onviewport).toHaveBeenLastCalledWith({ width: 50, height: 667, locked: false });
    expect(width?.value).toBe('50');
    button('Close device toolbar')?.click();
    expect(on.onmore).toHaveBeenCalledWith('device_toolbar');
  });
});
