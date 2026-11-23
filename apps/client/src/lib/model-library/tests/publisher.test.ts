import { describe, expect, it } from 'vitest';
import { publisherMark } from '../publisher';

describe('publisherMark', () => {
  it('gives the brand mark of a known publisher, in any case', () => {
    expect(publisherMark({ name: 'qwen3', publisher: 'Qwen' })).toEqual({ icon: 'simple-icons:qwen' });
    expect(publisherMark({ name: 'llama3.3', publisher: 'meta-llama' })).toEqual({ icon: 'simple-icons:meta' });
  });

  it('knows the family by the name when the library names no publisher', () => {
    expect(publisherMark({ name: 'gemma3', publisher: null })).toEqual({ icon: 'simple-icons:google' });
    expect(publisherMark({ name: 'gpt-oss', publisher: null })).toEqual({ icon: 'simple-icons:openai' });
  });

  it('falls back to the first letter of the publisher, else of the name', () => {
    expect(publisherMark({ name: 'nomic-embed', publisher: 'nomic-ai' })).toEqual({ letter: 'N' });
    expect(publisherMark({ name: 'tiny', publisher: null })).toEqual({ letter: 'T' });
  });
});
