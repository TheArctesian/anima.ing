import { describe, expect, it } from 'vitest';
import { runs } from './inline';

/** The runs of a line as words: styled text as `style:text`, a delimiter in angle brackets. */
function read(text: string): string[] {
  return runs(text).map((run) => {
    const piece = text.slice(run.start, run.end);
    if (run.marker) return `<${piece}>`;
    return run.styles.length === 0 ? piece : `${[...run.styles].sort().join('+')}:${piece}`;
  });
}

describe('the inline styles of a line', () => {
  it('reads bold, italic, strike and code, each with its delimiters', () => {
    expect(read('a **b** c')).toEqual(['a ', '<**>', 'bold:b', '<**>', ' c']);
    expect(read('__b__ and *i*')).toEqual(['<__>', 'bold:b', '<__>', ' and ', '<*>', 'italic:i', '<*>']);
    expect(read('~~gone~~')).toEqual(['<~~>', 'strike:gone', '<~~>']);
    expect(read('run `ls -l` now')).toEqual(['run ', '<`>', 'code:ls -l', '<`>', ' now']);
  });

  it('nests styles, and keeps Markdown inside code as text', () => {
    expect(read('***both***')).toEqual(['<***>', 'bold+italic:both', '<***>']);
    expect(read('*a **b** c*')).toEqual(['<*>', 'italic:a ', '<**>', 'bold+italic:b', '<**>', 'italic: c', '<*>']);
    expect(read('`a **b**`')).toEqual(['<`>', 'code:a **b**', '<`>']);
  });

  it('leaves what does not close, or stands alone, as text', () => {
    expect(read('**open')).toEqual(['**open']);
    expect(read('2 * 3 * 4')).toEqual(['2 * 3 * 4']);
    expect(read('a ~ b ~ c')).toEqual(['a ~ b ~ c']);
    expect(read('``two`` ticks')).toEqual(['``two`` ticks']);
    expect(read('****')).toEqual(['****']);
  });

  it('keeps underscores in names, paths and words as text', () => {
    expect(read('see node_modules and my_file.ts')).toEqual(['see node_modules and my_file.ts']);
    expect(read('__init__.py')).toEqual(['__init__.py']);
    expect(read('src/my_app/a_b.ts')).toEqual(['src/my_app/a_b.ts']);
    expect(read('an _aside_ here')).toEqual(['an ', '<_>', 'italic:aside', '<_>', ' here']);
  });

  it('keeps the character after a backslash as text', () => {
    expect(read(String.raw`\*not\*`)).toEqual([String.raw`\*not\*`]);
  });

  it('gives every character to one run, and one region to a styled stretch with its delimiters', () => {
    const text = 'a **b** *c* d';
    const found = runs(text);
    expect(found.map((run) => text.slice(run.start, run.end)).join('')).toBe(text);
    expect(found.map((run) => run.region)).toEqual([null, { start: 2, end: 7 }, { start: 2, end: 7 }, { start: 2, end: 7 }, null, { start: 8, end: 11 }, { start: 8, end: 11 }, { start: 8, end: 11 }, null]);
  });
});
