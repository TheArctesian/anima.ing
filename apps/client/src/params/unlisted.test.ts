import { describe, expect, it } from 'vitest';
import { HOSTING_SECTIONS } from '$lib/hosting/sections/sections';
import { match } from './unlisted';

describe('the sections of Hosting that one page serves', () => {
  it('HSE-001: Models opens it, and other names under /hosting are not found', () => {
    expect(['models'].map(match)).toEqual([true]);
    expect(['unknown', 'toString', 'routes', 'storage', 'machines', ''].map(match)).toEqual([false, false, false, false, false, false]);
  });

  it('HSE-001: each section without a page of its own has its words', () => {
    const own = new Set(['/hosting', '/hosting/catalogue', '/hosting/storage', '/hosting/routes', '/hosting/machines']);
    const served = HOSTING_SECTIONS.filter((section) => !own.has(section.path)).map((section) => section.path.replace('/hosting/', ''));
    expect(served.every(match)).toBe(true);
  });
});
