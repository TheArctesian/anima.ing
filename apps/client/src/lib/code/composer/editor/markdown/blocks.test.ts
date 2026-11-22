import { describe, expect, it } from 'vitest';
import { checkAt, lineIndexAt, lines } from './blocks';

function kinds(markdown: string, mode: 'rich' | 'plain' | 'literal' = 'rich'): string[] {
  return lines(markdown, mode).map((line) => line.kind);
}

describe('the lines of a prompt', () => {
  it('reads headings, lists, quotes and rules by the marker at the start of the line', () => {
    const found = lines('# One\n###### Six\n- a\n3) b\n- [x] c\n> d\n---\ntext', 'rich');
    expect(found.map((line) => [line.kind, line.prefix])).toEqual([
      ['heading', 2],
      ['heading', 7],
      ['bullet', 2],
      ['ordered', 3],
      ['task', 6],
      ['quote', 2],
      ['rule', 3],
      ['text', 0],
    ]);
    expect(found.map((line) => line.level).slice(0, 2)).toEqual([1, 6]);
    expect(found[3]?.marker).toBe('3)');
    expect(found[4]?.checked).toBe(true);
  });

  it('keeps a marker that is still being typed as text', () => {
    expect(kinds('#\n#hash\n-\n*\n1.\n- [x]y\n--\n####### seven')).toEqual(['text', 'text', 'text', 'text', 'text', 'bullet', 'text', 'text']);
  });

  it('gives every line its place in the prompt', () => {
    const found = lines('ab\n\ncd', 'rich');
    expect(found.map((line) => line.start)).toEqual([0, 3, 4]);
    expect([0, 2, 3, 4, 6].map((at) => lineIndexAt(found, at))).toEqual([0, 0, 1, 2, 2]);
  });

  it('nests list lines by their indent', () => {
    expect(lines('- a\n  - b\n    1. c\n  - d\n- e\n\n  - f', 'rich').map((line) => line.depth)).toEqual([0, 1, 2, 1, 0, 0, 0]);
  });

  it('reads a fenced code block, with its Markdown as code', () => {
    const found = lines('```ts\n# no heading\n- no list\n```\nafter', 'rich');
    expect(found.map((line) => line.kind)).toEqual(['fence', 'code', 'code', 'close', 'text']);
    expect(found.map((line) => line.block)).toEqual([0, 0, 0, 0, -1]);
    expect(found[0]?.prefix).toBe(3);
  });

  it('closes a block only with its own fence, and runs an open block to the end', () => {
    expect(kinds('````\n```\n````')).toEqual(['fence', 'code', 'close']);
    expect(kinds('~~~\n```\n~~~')).toEqual(['fence', 'code', 'close']);
    expect(kinds('```\na\n\nb')).toEqual(['fence', 'code', 'code', 'code']);
    expect(kinds('```a`b\ntext')).toEqual(['text', 'text']);
  });

  it('reads no blocks in plain and literal text', () => {
    expect(kinds('# a\n- b\n```\nc', 'plain')).toEqual(['text', 'text', 'text', 'text']);
    expect(kinds('# a\n> b', 'literal')).toEqual(['text', 'text']);
  });
});

describe('what the drawing of a block needs', () => {
  it('marks where a list, a nested list, a quote and a code block start and end', () => {
    const edges = lines('p\n- a\n  - b\n- c\n> q\n> r\n```\nx\n```', 'rich').map((line) => `${line.first ? 'first' : ''} ${line.last ? 'last' : ''}`.trim());
    expect(edges).toEqual(['', 'first', 'first last', 'last', 'first', 'last', 'first', '', 'last']);
  });

  it('gives a list no space of its own under it when an empty line follows', () => {
    expect(lines('- a\n\ntext', 'rich')[0]?.last).toBe(false);
    expect(lines('- a\ntext', 'rich')[0]?.last).toBe(true);
  });

  it('gives the ordered lines of a list the width of its widest marker', () => {
    expect(lines('9. a\n10. b\n   1. c\ntext\n1. d', 'rich').map((line) => line.gutter)).toEqual([3, 3, 2, 0, 2]);
  });

  it('finds the box of a task', () => {
    expect(checkAt('- [ ] a')).toBe(3);
    expect(checkAt('  -  [x] a')).toBe(6);
    expect(checkAt('- [ ]')).toBe(3);
    expect(checkAt('- [ ]a')).toBe(-1);
    expect(checkAt('* [ ] a')).toBe(-1);
  });
});
