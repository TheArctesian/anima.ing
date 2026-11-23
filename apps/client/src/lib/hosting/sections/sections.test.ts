import { describe, expect, it } from 'vitest';
import { HOSTING_SECTIONS, sectionOf } from './sections';

describe('the sections of Hosting', () => {
  it('HSE-001: are in the ruled order, with the Catalogue below Applications', () => {
    expect(HOSTING_SECTIONS.map((section) => `${section.below === true ? '  ' : ''}${section.title}`)).toEqual(['Applications', '  Catalogue', 'Models', 'Storage', 'Routes', 'Machines']);
  });

  it('HSE-001: each address opens its own section, and any other address of Hosting opens Applications', () => {
    expect(sectionOf('/hosting').title).toBe('Applications');
    expect(sectionOf('/hosting/routes').title).toBe('Routes');
    expect(sectionOf('/hosting/routes/').title).toBe('Routes');
    expect(sectionOf('/hosting/catalogue/jellyfin').title).toBe('Catalogue');
    expect(sectionOf('/hosting/unknown').title).toBe('Applications');
  });
});
