import { describe, expect, it } from 'vitest';
import { bestFit, capabilityWords, factsLine, fitBadge, shortParameters, shortTokens } from '../fit';
import { LLAMA, QWEN, fit } from './library.fixture';

describe('the words of a fit (FIT-005)', () => {
  it('says where the model runs, or what it lacks, with the tone of its status', () => {
    const shapes = [
      fit('a:b', 'pop-os', { state: 'FITS', placement: 'GPU' }),
      fit('a:b', 'pop-os', { state: 'FITS', placement: 'PARTIAL' }),
      fit('a:b', 'pop-os', { state: 'FITS', placement: 'CPU' }),
      fit('a:b', 'pop-os', { state: 'REDUCED_CONTEXT', largestContext: 16384 }),
      fit('a:b', 'pop-os', { state: 'TOO_LARGE' }),
      fit('a:b', 'pop-os', { state: 'UNKNOWN' }),
    ];
    expect(shapes.map(fitBadge)).toEqual([
      { words: 'Fits on the GPU', tone: 'up' },
      { words: 'Partly on the GPU', tone: 'degraded' },
      { words: 'CPU only', tone: 'unknown' },
      { words: 'Fits at 16K', tone: 'degraded' },
      { words: 'Too large', tone: 'down' },
      { words: 'Fit not known', tone: 'unknown' },
    ]);
  });

  it('ranks the GPU first, then partly on it, the CPU, a smaller context, too large and not known', () => {
    const cpu = fit('a:b', 'nas', { state: 'FITS', placement: 'CPU' });
    const reduced = fit('a:b', 'pop-os', { state: 'REDUCED_CONTEXT', largestContext: 4096 });
    const gpu = fit('a:b', 'mini', { state: 'FITS', placement: 'GPU' });
    expect(bestFit([reduced, cpu])).toBe(cpu);
    expect(bestFit([reduced, cpu, gpu])).toBe(gpu);
    expect(bestFit([])).toBeNull();
  });
});

describe('the facts of a model as a list shows them (SEL-005, LIBM-008)', () => {
  it('writes the parameters, the context and the tags as people write them', () => {
    expect([shortParameters(8.2e9), shortParameters(137e6), shortParameters(405e9)]).toEqual(['8.2B', '137M', '405B']);
    expect([shortTokens(8192), shortTokens(131072), shortTokens(512)]).toEqual(['8K', '128K', '512']);
    expect([capabilityWords('IMAGE_UNDERSTANDING'), capabilityWords('OCR')]).toEqual(['Image understanding', 'OCR']);
    expect(factsLine(QWEN)).toBe('Qwen · 8.2B · 32K');
    expect(factsLine(LLAMA)).toBe('Meta · 70B · 128K');
  });
});
