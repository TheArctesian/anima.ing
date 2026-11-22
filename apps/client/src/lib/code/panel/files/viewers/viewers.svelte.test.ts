// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import ImageViewer from './ImageViewer.svelte';
import SourceView from './SourceView.svelte';

let app: ReturnType<typeof mount> | undefined;

afterEach(() => {
  if (app !== undefined) void unmount(app);
  app = undefined;
  document.body.innerHTML = '';
});

const named = (label: string): HTMLButtonElement => {
  const button = document.querySelector<HTMLButtonElement>(`button[aria-label="${label}"]`);
  if (!button) throw new Error(`no button "${label}"`);
  return button;
};

function click(label: string): void {
  named(label).click();
  flushSync();
}

/** The image "loads" with a natural size, as a browser would say it. */
function load(width: number, height: number): HTMLImageElement {
  const image = document.querySelector('img');
  if (!image) throw new Error('no image');
  Object.defineProperty(image, 'naturalWidth', { value: width });
  Object.defineProperty(image, 'naturalHeight', { value: height });
  image.dispatchEvent(new Event('load'));
  flushSync();
  return image;
}

describe('the image viewer', () => {
  it('says the size when it is known, and fits the image at first', () => {
    const onexpand = vi.fn();
    app = mount(ImageViewer, { target: document.body, props: { src: 'blob:a', name: 'shot.png', onexpand } });
    flushSync();
    expect(document.querySelector('.size')?.textContent).toBe('Loading image…');
    load(1920, 1080);
    expect(document.querySelector('.size')?.textContent).toBe('1,920 × 1,080');
    expect(named('Fit image').getAttribute('aria-pressed')).toBe('true');
    named('Zoom shot.png').click();
    expect(onexpand).toHaveBeenCalledOnce();
  });

  it('zooms by steps of 1.25 and stops at 5% and 800%', () => {
    app = mount(ImageViewer, { target: document.body, props: { src: 'blob:a', name: 'shot.png' } });
    flushSync();
    const image = load(200, 100);
    const shown = (): string | undefined => document.querySelector('.percent')?.textContent;
    expect(shown()).toBe('100%');
    expect(named('Actual image size').getAttribute('aria-pressed')).toBe('true');
    click('Zoom in');
    expect(shown()).toBe('125%');
    expect(image.style.inlineSize).toBe('250px');
    expect(named('Fit image').getAttribute('aria-pressed')).toBe('false');
    expect(named('Actual image size').getAttribute('aria-pressed')).toBe('false');
    for (let step = 0; step < 12; step += 1) click('Zoom in');
    expect(shown()).toBe('800%');
    expect(named('Zoom in').disabled).toBe(true);
    click('Actual image size');
    expect(shown()).toBe('100%');
    for (let step = 0; step < 16; step += 1) click('Zoom out');
    expect(shown()).toBe('5%');
    expect(named('Zoom out').disabled).toBe(true);
    click('Fit image');
    expect(named('Fit image').getAttribute('aria-pressed')).toBe('true');
  });

  it('says that the image did not load, and asks again', () => {
    const onretry = vi.fn();
    app = mount(ImageViewer, { target: document.body, props: { src: 'blob:a', name: 'shot.png', onretry } });
    flushSync();
    document.querySelector('img')?.dispatchEvent(new Event('error'));
    flushSync();
    expect(document.querySelector('[role="alert"] p')?.textContent).toBe('Unable to load image.');
    [...document.querySelectorAll('button')].find((button) => button.textContent === 'Try again')?.click();
    flushSync();
    expect(onretry).toHaveBeenCalledOnce();
    expect(document.querySelector('img')).not.toBeNull();
  });
});

describe('the source view', () => {
  const rows = (): HTMLElement[] => [...document.querySelectorAll<HTMLElement>('code .row')];
  const revealed = (): (string | undefined)[] => [...document.querySelectorAll<HTMLElement>('[data-file-link-reveal]')].map((row) => row.dataset.n);

  it('draws each line as text with its number, in a region that the keys can reach', () => {
    app = mount(SourceView, { target: document.body, props: { text: 'a <b>\n\nc', wrap: true } });
    flushSync();
    const region = document.querySelector('[role="region"]');
    expect(region?.getAttribute('aria-label')).toBe('File contents');
    expect(region?.getAttribute('tabindex')).toBe('0');
    expect(rows().map((row) => [row.dataset.n, row.textContent])).toEqual([['1', 'a <b>'], ['2', ''], ['3', 'c']]);
    expect(document.querySelector('code b')).toBeNull();
    expect(document.querySelector('pre')?.classList.contains('wrap')).toBe(true);
  });

  it('marks the asked line, held to the lines of the file', () => {
    const props = $state({ text: 'a\nb\nc', line: 2 as number | undefined });
    app = mount(SourceView, { target: document.body, props });
    flushSync();
    expect(revealed()).toEqual(['2']);
    props.line = 99;
    flushSync();
    expect(revealed()).toEqual(['3']);
    props.text = 'a';
    flushSync();
    expect(revealed()).toEqual(['1']);
    props.line = undefined;
    flushSync();
    expect(revealed()).toEqual([]);
  });
});
