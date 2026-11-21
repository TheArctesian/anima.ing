// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import type { Chip } from '../../draft';
import type { Span } from '../edit';
import { lines } from '../markdown/blocks';
import { paint, type Drawn } from './draw';
import { offsetOf, pointAt, read } from './points';
import { sketch } from './sketch';

const SOURCE = '[a](a.ts)';
let releases = 0;

function icon(target: HTMLElement, name: string): () => void {
  target.dataset.icon = name;
  return () => { releases += 1; };
}

function page(markdown: string, sel: Span | null = null, before: { root: HTMLElement; drawn: readonly Drawn[] } | null = null) {
  const root = before?.root ?? document.body.appendChild(document.createElement('div'));
  const at = markdown.indexOf(SOURCE);
  const chips: Chip[] = at === -1 ? [] : [{ start: at, end: at + SOURCE.length, label: 'a.ts', icon: 'file', detail: 'src/a.ts' }];
  const sketches = lines(markdown, 'rich').map((line) => sketch(line, { mode: 'rich', chips, sel }));
  return { root, ...paint(root, before?.drawn ?? [], sketches, icon) };
}

afterEach(() => {
  document.body.replaceChildren();
  releases = 0;
});

const PROMPT = '# Title\n\n- a **b** `c`\n  1. [x](y)\n- [ ] task\n> quote\n---\n```ts\n  code\n\n```\nlast *one*';

describe('the page and the prompt', () => {
  it('holds every character of the prompt, shown or hidden, so it reads back the same', () => {
    expect(read(page(PROMPT).root)).toBe(PROMPT);
    expect(read(page(PROMPT, { start: 0, end: PROMPT.length }).root)).toBe(PROMPT);
    expect(read(page('').root)).toBe('');
    expect(read(page('a\n\n').root)).toBe('a\n\n');
  });

  it('gives every offset of shown text a place, and reads that place back as the offset', () => {
    const shown = 'a **b** `c`\n\n```ts\n  code\n```\nlast *one* [a](a.ts) end';
    const { root } = page(shown, { start: 0, end: shown.length });
    const chip = { start: shown.indexOf(SOURCE), end: shown.indexOf(SOURCE) + SOURCE.length };
    for (let offset = 0; offset <= shown.length; offset += 1) {
      const point = pointAt(root, offset);
      const inChip = offset > chip.start && offset < chip.end;
      expect(offsetOf(root, point.node, point.offset)).toBe(inChip ? chip.end : offset);
    }
  });

  it('gives an offset in a hidden marker the place after it', () => {
    const { root } = page(PROMPT);
    for (let offset = 0; offset <= PROMPT.length; offset += 1) {
      const point = pointAt(root, offset);
      const back = offsetOf(root, point.node, point.offset) ?? -1;
      expect(back).toBeGreaterThanOrEqual(offset);
      expect(PROMPT.slice(offset, back)).not.toContain('\n');
    }
  });

  it('puts the caret after a hidden marker, where the text starts', () => {
    const { root } = page('## Title\n- \n- [ ] ');
    const title = pointAt(root, 3);
    expect(title.node.nodeValue).toBe('Title');
    expect(title.offset).toBe(0);
    const empty = pointAt(root, 11);
    expect(empty.node).toBe(root.children[1]);
    expect(empty.node.childNodes[empty.offset]?.nodeName).toBe('BR');
    expect(offsetOf(root, empty.node, empty.offset)).toBe(11);
  });

  it('reads a place between the lines, as select all gives it', () => {
    const { root } = page('ab\ncd\nef');
    expect([0, 1, 2, 3].map((line) => offsetOf(root, root, line))).toEqual([0, 3, 6, 8]);
    expect(offsetOf(root, document.body, 0)).toBeNull();
  });

  it('takes a chip as one position: a place inside it is one of its ends', () => {
    const { root } = page('a [a](a.ts) b');
    const chip = root.querySelector<HTMLElement>('.ce-chip');
    const label = chip?.querySelector('.ce-chip-label')?.firstChild ?? null;
    expect(chip?.dataset.source).toBe('[a](a.ts)');
    expect(chip?.getAttribute('contenteditable')).toBe('false');
    expect(label === null ? null : offsetOf(root, label, 0)).toBe(2);
    expect(label === null ? null : offsetOf(root, label, 2)).toBe(11);
    expect(pointAt(root, 5).node).not.toBe(label);
    expect(read(root)).toBe('a [a](a.ts) b');
  });

  it('reads back what an input method typed into a line', () => {
    const { root } = page('- item\nnext');
    const text = pointAt(root, 6);
    (text.node as Text).insertData(text.offset, 'あ');
    expect(read(root)).toBe('- itemあ\nnext');
  });
});

describe('drawing again', () => {
  it('keeps the lines that did not change, and frees the icons of those that did', () => {
    const first = page('one\na [a](a.ts) b\nthree');
    const kept = [first.root.children[0], first.root.children[2]];
    const second = page('one\na [a](a.ts) b!\nmore\nthree', null, first);
    expect(second.changed).toBe(true);
    expect([second.root.children[0], second.root.children[3]]).toEqual(kept);
    expect(read(second.root)).toBe('one\na [a](a.ts) b!\nmore\nthree');
    expect(releases).toBe(1);
    expect(page('one\na [a](a.ts) b!\nmore\nthree', null, second).changed).toBe(false);
  });

  it('draws a task with a real checkbox, and a list line with its marker as written', () => {
    const { root } = page('- [x] done\n3) third\n  - deep');
    const box = root.querySelector<HTMLInputElement>('input.ce-check');
    expect(box?.type).toBe('checkbox');
    expect(box?.checked).toBe(true);
    expect((root.children[1] as HTMLElement).dataset.marker).toBe('3)');
    expect((root.children[2] as HTMLElement).style.getPropertyValue('--code-composer-editor-depth')).toBe('1');
  });

  it('gives each line its own direction, and code left to right', () => {
    const { root } = page('مرحبا\n```\nx\n```\n`c`', { start: 0, end: 99 });
    expect(Array.from(root.children, (line) => (line as HTMLElement).dir)).toEqual(['auto', 'ltr', 'ltr', 'ltr', 'auto']);
    expect(root.querySelector('code')?.dir).toBe('ltr');
  });
});
