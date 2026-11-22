import { expect, test, type Page } from '@playwright/test';
import { HUB_ORIGIN } from '../../synthetic/access';
import { installSyntheticHub } from '../../synthetic/synthetic-hub';

// The composer's editor with an input method, in WebKit (the engine of the Mac application). No browser test can
// run a real input method, so each composition here is the sequence of events that WebKit sends, with the page
// changed between them as the browser changes it. It needs a build made with `VITE_ANIMA_SPECIMEN=1` and runs
// only when asked, `GALLERY_CODE_COMPOSER=1`. It makes no pictures.

const BOX = '[data-kit="rest"] [role="textbox"]';

interface Composition {
  /** What the method shows at each key, in order: "k", "か", "かな". */
  readonly steps: readonly string[];
  /** The words it puts in at the end; empty with `cancel`. */
  readonly final: string;
  /** `webkit`: the composed text is deleted and put in again, then the composition ends. `chromium`: it only ends. */
  readonly ending: 'webkit' | 'chromium' | 'cancel';
  /** The key that ends it, which WebKit sends after the end with code 229. */
  readonly endKey: 'Enter' | 'Escape';
  /** A button to press when the method shows its last step, before the end. */
  readonly midway?: string;
}

interface Composed {
  /** A key of the composition that the page refused: the editor took a key that was the method's. */
  readonly refused: readonly string[];
  /** The text node of the composition was still in the page just before the end. */
  readonly kept: boolean;
}

/** Runs in the page: one composition at the caret of the first composer's editor. */
async function composeInPage({ box: selector, steps, final, ending, endKey, midway }: Composition & { box: string }): Promise<Composed> {
  const wait = (ms: number): Promise<void> => new Promise((done) => setTimeout(done, ms));
  const box = document.querySelector<HTMLElement>(selector);
  const selection = getSelection();
  if (box === null || selection?.anchorNode == null) throw new Error('no caret in the editor');
  const refused: string[] = [];
  const key = (type: string, name: string, keyCode: number, isComposing: boolean): void => {
    if (!box.dispatchEvent(new KeyboardEvent(type, { key: name, keyCode, isComposing, bubbles: true, cancelable: true }))) refused.push(`${type} ${name}`);
  };
  const told = (type: string, data: string): boolean => box.dispatchEvent(new CompositionEvent(type, { data, bubbles: true }));
  const input = (inputType: string, data: string | null, change: () => void): void => {
    box.dispatchEvent(new InputEvent('beforeinput', { inputType, data, isComposing: true, bubbles: true, cancelable: false }));
    change();
    box.dispatchEvent(new InputEvent('input', { inputType, data, isComposing: true, bubbles: true }));
  };
  // The browser writes into the text at the caret; on an empty line it makes the text and takes the line's <br> away.
  const home = (node: Node, offset: number): { text: Text; at: number } => {
    if (node instanceof Text) return { text: node, at: offset };
    const next = node.childNodes[offset] ?? null;
    const made = node.insertBefore(document.createTextNode(''), next);
    if (next?.nodeName === 'BR') next.remove();
    return { text: made, at: 0 };
  };
  const { text, at } = home(selection.anchorNode, selection.anchorOffset);
  let length = 0;
  const put = (words: string) => () => {
    text.replaceData(at, length, words);
    length = words.length;
    selection.collapse(text, at + length);
  };
  steps.forEach((step, index) => {
    // The first key goes down before the composition starts; each has code 229.
    key('keydown', 'Process', 229, index > 0);
    if (index === 0) told('compositionstart', '');
    told('compositionupdate', step);
    input('insertCompositionText', step, put(step));
    key('keyup', 'Process', 229, true);
  });
  if (midway !== undefined) {
    document.querySelector<HTMLElement>(midway)?.click();
    await wait(50);
  }
  const kept = text.isConnected;
  if (ending === 'chromium') {
    key('keydown', endKey, 229, true);
    told('compositionupdate', final);
    input('insertCompositionText', final, put(final));
    told('compositionend', final);
  } else {
    input('deleteCompositionText', null, put(''));
    if (ending === 'webkit') input('insertFromComposition', final, put(final));
    told('compositionend', final);
    // WebKit sends the key that ended the composition after the end, with code 229 and `isComposing` false.
    key('keydown', endKey, 229, false);
  }
  key('keyup', endKey, endKey === 'Enter' ? 13 : 27, false);
  await wait(50);
  return { refused, kept };
}

/** Runs in the page: the prompt as Markdown, read from the page as the editor reads it. */
function markdownInPage(selector: string): string {
  const textOf = (node: Node): string => {
    if (node.nodeType === Node.TEXT_NODE) return node.nodeValue ?? '';
    if (node instanceof HTMLElement && node.dataset['source'] !== undefined) return node.dataset['source'];
    return Array.from(node.childNodes, textOf).join('');
  };
  return Array.from(document.querySelector(selector)?.childNodes ?? [], textOf).join('\n');
}

/** Runs in the page: puts the caret `offset` characters into the first shown text that holds `words`. */
async function caretInPage({ box, words, offset }: { box: string; words: string; offset: number }): Promise<void> {
  const walker = document.createTreeWalker(document.querySelector(box) ?? document.body, NodeFilter.SHOW_TEXT);
  let found: Node | null = null;
  while (found === null && walker.nextNode() !== null) if (walker.currentNode.nodeValue?.includes(words) === true) found = walker.currentNode;
  if (found === null) throw new Error(`no text ${words}`);
  getSelection()?.collapse(found, (found.nodeValue ?? '').indexOf(words) + offset);
  // The editor follows the selection and may draw the line again, with its markers shown.
  await new Promise((done) => setTimeout(done, 50));
}

const markdown = (page: Page): Promise<string> => page.evaluate(markdownInPage, BOX);
const compose = (page: Page, composition: Composition): Promise<Composed> => page.evaluate(composeInPage, { box: BOX, ...composition });
const caret = (page: Page, words: string, offset: number): Promise<void> => page.evaluate(caretInPage, { box: BOX, words, offset });
const KANA: Composition = { steps: ['k', 'か', 'かn', 'かな'], final: '仮名', ending: 'webkit', endKey: 'Enter' };

/** The page as a send leaves it: the composer empties the prompt from outside while the method still composes. */
const SEND = '[data-kit="rest"] button[aria-label="Submit message"]';

/** Types lines into the editor; Shift+Enter breaks a line, as Enter alone sends. */
async function write(page: Page, lines: readonly string[]): Promise<void> {
  await page.locator(BOX).click();
  for (const [index, line] of lines.entries()) {
    if (index > 0) await page.keyboard.press('Shift+Enter');
    await page.keyboard.type(line);
  }
}

/** Where the caret is, shown by a character typed there. */
async function marked(page: Page): Promise<string> {
  await page.keyboard.type('|');
  return markdown(page);
}

test.skip(process.env['GALLERY_CODE_COMPOSER'] !== '1', 'The composer is driven only when asked.');

test.beforeEach(async ({ page, context }) => {
  const hub = await installSyntheticHub(context, {});
  await hub.signIn();
  await page.goto(`${HUB_ORIGIN}/code-kit/composer`);
  await expect(page.getByRole('heading', { level: 1, name: 'Code kit: composer' })).toBeVisible();
});

test.describe('code-composer-input-method', () => {
  test('a composition in an empty editor puts its words in, with the caret after them', async ({ page }) => {
    await page.locator(BOX).click();
    const done = await compose(page, KANA);
    expect(done).toEqual({ refused: [], kept: true });
    expect(await markdown(page)).toBe('仮名');
    expect(await marked(page)).toBe('仮名|');
  });

  test('a composition that ends as Chromium ends it gives the same prompt', async ({ page }) => {
    await page.locator(BOX).click();
    await compose(page, { ...KANA, ending: 'chromium' });
    expect(await marked(page)).toBe('仮名|');
  });

  test('a composition in the middle of a line goes in at the caret', async ({ page }) => {
    await write(page, ['one three']);
    await caret(page, 'one three', 4);
    await compose(page, { steps: ['ㅎ', '하', '한'], final: '한', ending: 'webkit', endKey: 'Enter' });
    expect(await marked(page)).toBe('one 한|three');
  });

  test('a composition inside bold text and inside inline code stays inside the markers', async ({ page }) => {
    await write(page, ['a **bold** and `code` z']);
    await caret(page, 'bold', 2);
    await compose(page, KANA);
    expect(await markdown(page)).toBe('a **bo仮名ld** and `code` z');
    await caret(page, 'code', 4);
    await compose(page, { steps: ['n', 'ni', '你'], final: '你', ending: 'webkit', endKey: 'Enter' });
    expect(await marked(page)).toBe('a **bo仮名ld** and `code你|` z');
    await expect(page.locator(BOX).locator('.ce-bold')).toHaveText('bo仮名ld');
    await expect(page.locator(BOX).locator('code')).toHaveText('code你|');
  });

  test('a composition at the start of a list line keeps the line a list line', async ({ page }) => {
    await write(page, ['- first', '']);
    expect(await markdown(page)).toBe('- first\n- ');
    await compose(page, KANA);
    expect(await markdown(page)).toBe('- first\n- 仮名');
    await caret(page, 'first', 0);
    await compose(page, KANA);
    expect(await marked(page)).toBe('- 仮名|first\n- 仮名');
    await expect(page.locator(BOX).locator('.ce-bullet')).toHaveCount(2);
  });

  test('a cancelled composition leaves the prompt as it was, and the editor can be typed in', async ({ page }) => {
    await page.locator(BOX).click();
    const done = await compose(page, { steps: ['k', 'か'], final: '', ending: 'cancel', endKey: 'Escape' });
    expect(done.refused).toEqual([]);
    expect(await markdown(page)).toBe('');
    await expect(page.locator(BOX).locator('.ce-line > br')).toHaveCount(1);
    await write(page, ['ab']);
    await caret(page, 'ab', 1);
    await compose(page, { steps: ['k', 'か'], final: '', ending: 'cancel', endKey: 'Escape' });
    expect(await marked(page)).toBe('a|b');
  });

});

test.describe('code-composer-input-method-and-send', () => {
  test('the Enter that ends a composition does not send the message; the next Enter does', async ({ page }) => {
    await write(page, ['hello ']);
    const done = await compose(page, KANA);
    expect(done.refused).toEqual([]);
    expect(await markdown(page)).toBe('hello 仮名');
    await compose(page, { ...KANA, ending: 'chromium' });
    expect(await markdown(page)).toBe('hello 仮名仮名');
    await page.keyboard.press('Enter');
    await expect.poll(() => markdown(page)).toBe('');
  });

  test('a prompt emptied from outside during a composition is not drawn until the end, and the composed words stay', async ({ page }) => {
    await write(page, ['sent']);
    const done = await compose(page, { ...KANA, midway: SEND });
    expect(done.kept).toBe(true);
    expect(await marked(page)).toBe('仮名|');
  });

});

test.describe('code-composer-keys', () => {
  test('typing, Backspace over a list marker and over a chip', async ({ page }) => {
    await write(page, ['- task']);
    await caret(page, 'task', 0);
    await page.keyboard.press('Backspace');
    expect(await marked(page)).toBe('|task');
    await page.keyboard.press('ControlOrMeta+a');
    await page.keyboard.type('Read @comp');
    await expect(page.getByRole('listbox').first()).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(page.locator(BOX).locator('.ce-chip')).toHaveCount(1);
    const withChip = await markdown(page);
    expect(withChip.startsWith('Read ')).toBe(true);
    // One Backspace for the space after the chip, where there is one, and one for the chip, whole.
    for (let presses = withChip.endsWith(' ') ? 2 : 1; presses > 0; presses -= 1) await page.keyboard.press('Backspace');
    await expect(page.locator(BOX).locator('.ce-chip')).toHaveCount(0);
    expect(await markdown(page)).toBe('Read ');
  });

  test('undo and redo, select all and replace, and a paste of plain text', async ({ page }) => {
    await write(page, ['one']);
    await page.keyboard.press('Meta+b');
    expect(await markdown(page)).toBe('one****');
    await page.keyboard.press('Meta+z');
    expect(await markdown(page)).toBe('one');
    await page.keyboard.press('Shift+Meta+z');
    expect(await markdown(page)).toBe('one****');
    await page.keyboard.press('Meta+a');
    await page.keyboard.type('x');
    expect(await markdown(page)).toBe('x');
    await page.evaluate((selector) => {
      const clipboardData = new DataTransfer();
      clipboardData.setData('text/plain', ' and\r\n**pasted**');
      document.querySelector(selector)?.dispatchEvent(new ClipboardEvent('paste', { clipboardData, bubbles: true, cancelable: true }));
    }, BOX);
    expect(await marked(page)).toBe('x and\n**pasted**|');
    // The composed words are typing: one undo takes them with the character typed before them.
    await compose(page, KANA);
    expect(await markdown(page)).toBe('x and\n**pasted**|仮名');
    await page.keyboard.press('Meta+z');
    expect(await markdown(page)).toBe('x and\n**pasted**');
    await page.keyboard.press('Shift+Meta+z');
    expect(await markdown(page)).toBe('x and\n**pasted**|仮名');
  });

});

test.describe('code-composer-tab', () => {
  test('Tab in a code block moves the line; Escape, then Tab, leaves the editor', async ({ page }) => {
    await write(page, ['```', 'code']);
    expect(await markdown(page)).toBe('```\ncode\n```');
    await page.keyboard.press('Tab');
    expect(await markdown(page)).toBe('```\ncode  \n```');
    await expect(page.locator(BOX)).toBeFocused();
    // Another key after Escape: the Tab is the editor's again.
    await page.keyboard.press('Escape');
    await page.keyboard.press('ArrowLeft');
    await page.keyboard.press('Tab');
    const moved = '```\ncode    \n```';
    expect(await markdown(page)).toBe(moved);
    await expect(page.locator(BOX)).toBeFocused();
    await page.keyboard.press('Escape');
    await page.keyboard.press('Tab');
    await expect(page.locator(BOX)).not.toBeFocused();
    expect(await markdown(page)).toBe(moved);
  });

  test('Tab on a list line nests it; Escape, then Tab, leaves the editor, and the text box says so', async ({ page }) => {
    await write(page, ['- one', 'two']);
    await page.keyboard.press('Tab');
    expect(await markdown(page)).toBe('- one\n  - two');
    await page.keyboard.press('Escape');
    await page.keyboard.press('Tab');
    await expect(page.locator(BOX)).not.toBeFocused();
    expect(await markdown(page)).toBe('- one\n  - two');
    await expect(page.locator(BOX)).toHaveAccessibleDescription('In a list or a code block Tab indents. Press Escape, then Tab, to leave the editor.');
  });
});
