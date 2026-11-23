import { describe, expect, it } from 'vitest';
import { HOSTING_SECTIONS } from '$lib/hosting/sections/sections';
import { match } from './copies';

describe('the address of Hosting > Models', () => {
  it('HSE-013: only "models" opens the page of the copies', () => {
    expect(match('models')).toBe(true);
    expect(['unknown', 'toString', 'copies', 'storage', ''].map(match)).toEqual([false, false, false, false, false]);
  });

  it('HSE-001: each section of Hosting has a page of its own', () => {
    const own = new Set(['/hosting', '/hosting/catalogue', '/hosting/storage', '/hosting/routes', '/hosting/machines']);
    const served = HOSTING_SECTIONS.filter((section) => !own.has(section.path)).map((section) => section.path.replace('/hosting/', ''));
    expect(served.every(match)).toBe(true);
  });
});
