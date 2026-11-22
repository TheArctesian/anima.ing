// @vitest-environment jsdom
import { flushSync, mount, unmount } from 'svelte';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { applyReader, DEFAULT_READER, readerTokens, storedReader, storeReader } from './reader';
import ReaderRows from './ReaderRows.svelte';

const root = document.documentElement;

beforeEach(() => {
  localStorage.clear();
  root.removeAttribute('data-code-width');
  root.removeAttribute('style');
});

describe('the saved reader settings', () => {
  it('reads the defaults when nothing valid is saved: a width not offered, a contrast under 100 or off its step, a size under 14 px', () => {
    expect(storedReader(localStorage)).toEqual(DEFAULT_READER);
    localStorage.setItem('anima.code.width', 'huge');
    localStorage.setItem('anima.code.contrast', '50');
    localStorage.setItem('anima.code.font-size', '12');
    expect(storedReader(localStorage)).toEqual(DEFAULT_READER);
    localStorage.setItem('anima.code.contrast', '103');
    expect(storedReader(localStorage).contrast).toBe(100);
  });

  it('keeps a choice across a restart, and forgets a value set back to its default', () => {
    storeReader(localStorage, { width: 'wide', contrast: 150, fontSize: 16 });
    expect(storedReader(localStorage)).toEqual({ width: 'wide', contrast: 150, fontSize: 16 });
    storeReader(localStorage, { ...DEFAULT_READER, width: 'full' });
    expect(localStorage.getItem('anima.code.contrast')).toBeNull();
    expect(storedReader(localStorage)).toEqual({ ...DEFAULT_READER, width: 'full' });
  });

  it('reads the defaults when the storage refuses to be read', () => {
    const refusing = { getItem: () => { throw new Error('denied'); } } as unknown as Storage;
    expect(storedReader(refusing)).toEqual(DEFAULT_READER);
  });
});

describe('the tokens of the Code page', () => {
  it('only grows text and only adds contrast, so 12 px and 4.5:1 hold at each value (R-354)', () => {
    expect(readerTokens(DEFAULT_READER)).toEqual({ width: 'comfortable', textScale: '1.0000', contrastMix: '0%' });
    expect(readerTokens({ width: 'full', contrast: 200, fontSize: 20 })).toEqual({ width: 'full', textScale: '1.4286', contrastMix: '100%' });
    expect(readerTokens({ ...DEFAULT_READER, contrast: 150 }).contrastMix).toBe('50%');
  });

  it('marks the document root', () => {
    applyReader(root, { width: 'wide', contrast: 120, fontSize: 15 });
    expect(root.dataset['codeWidth']).toBe('wide');
    expect(root.style.getPropertyValue('--code-text-scale')).toBe('1.0714');
    expect(root.style.getPropertyValue('--code-contrast')).toBe('20%');
  });
});

describe('the rows in Settings, Code appearance', () => {
  let app: ReturnType<typeof mount> | undefined;

  afterEach(async () => {
    if (app !== undefined) await unmount(app);
    app = undefined;
    document.body.innerHTML = '';
  });

  function show(): HTMLElement {
    const target = document.createElement('div');
    document.body.append(target);
    app = mount(ReaderRows, { target });
    flushSync();
    return target;
  }

  const select = (target: HTMLElement, id: string): HTMLSelectElement => target.querySelector<HTMLSelectElement>(`#${id}`) ?? expect.fail(id);

  it('applies and saves a chat width, and offers a reset only while the value is not the default', () => {
    const target = show();
    expect(target.querySelector('[aria-label="Reset chat width to default"]')).toBeNull();
    const width = select(target, 'code-chat-width');
    width.value = 'wide';
    width.dispatchEvent(new Event('change', { bubbles: true }));
    flushSync();
    expect(root.dataset['codeWidth']).toBe('wide');
    expect(localStorage.getItem('anima.code.width')).toBe('wide');
    target.querySelector<HTMLButtonElement>('[aria-label="Reset chat width to default"]')?.click();
    flushSync();
    expect(root.dataset['codeWidth']).toBe('comfortable');
    expect(localStorage.getItem('anima.code.width')).toBeNull();
  });

  it('moves the contrast with the slider and writes the value beside it', () => {
    const target = show();
    const slider = target.querySelector<HTMLInputElement>('#code-contrast') ?? expect.fail('slider');
    expect(slider.min).toBe('100');
    slider.value = '140';
    slider.dispatchEvent(new Event('input', { bubbles: true }));
    flushSync();
    expect(target.querySelector('output')?.textContent).toBe('140%');
    expect(root.style.getPropertyValue('--code-contrast')).toBe('40%');
  });

  it('offers font sizes from 14 to 20 px', () => {
    const sizes = [...select(show(), 'code-font-size').options].map((option) => option.textContent);
    expect(sizes).toEqual(['14 px (default)', '15 px', '16 px', '17 px', '18 px', '19 px', '20 px']);
  });
});
