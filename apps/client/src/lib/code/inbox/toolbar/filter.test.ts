import { describe, expect, it } from 'vitest';
import { picked, shownProjects } from './filter';

const PROJECTS = [
  { folder: '/w/anima', name: 'anima', count: 3 },
  { folder: '/w/zeta', name: 'zeta', count: 1 },
  { folder: null, name: 'No folder', count: 2 },
];

describe('the project filter (02 section 5.3)', () => {
  it('finds projects by name or folder', () => {
    expect(shownProjects(PROJECTS, ' ZE ').map((project) => project.name)).toEqual(['zeta']);
    expect(shownProjects(PROJECTS, '/w/').map((project) => project.name)).toEqual(['anima', 'zeta']);
    expect(shownProjects(PROJECTS, '')).toBe(PROJECTS);
  });

  it('a click picks one project; Shift adds or removes one; "All projects" clears the filter', () => {
    const one = picked(new Set(), '/w/anima', false);
    expect([...one]).toEqual(['/w/anima']);
    const two = picked(one, null, true);
    expect([...two]).toEqual(['/w/anima', null]);
    expect([...picked(two, '/w/anima', true)]).toEqual([null]);
    expect([...picked(two, '/w/zeta', false)]).toEqual(['/w/zeta']);
    expect(picked(two, undefined, false).size).toBe(0);
  });
});
