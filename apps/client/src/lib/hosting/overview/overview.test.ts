import { describe, expect, it } from 'vitest';
import { NOW, ROUTES } from '$lib/application-routes/tests/routes.fixture';
import { CONTAINERS, MACHINES } from '../hosting.fixture';
import type { Place } from '../list';
import { concerns, figures, type Reads } from './overview';

const PLACES: readonly Place[] = MACHINES.map((machine) => ({ ...machine, status: 'ready', problem: '', containers: CONTAINERS[machine.id] ?? [], updates: [] }));

function reads(change: Partial<Reads> = {}): Reads {
  return {
    hosting: { status: 'ready', problem: '', places: PLACES },
    routes: { status: 'ready', problem: '', routes: ROUTES },
    now: NOW,
    ...change,
  };
}

describe('the numbers of the overview (HSE-019)', () => {
  it('counts the machines up, the applications running and the routes proven, and says how many need you', () => {
    expect(figures(reads()).map(({ title, value, word, needsYou }) => [title, value, word, needsYou])).toEqual([
      ['Machines up', '2', '1 needs you', true],
      ['Applications running', '2', '1 needs you', true],
      ['Routes proven', '3', '3 need you', true],
    ]);
  });

  it('says of how many when nothing needs you, so the word keeps its colour', () => {
    const up = PLACES.filter((place) => place.state === 'UP' && place.id === 'm-pop');
    const proven = ROUTES.filter((route) => route.id === 'route-vault');
    const shown = figures(reads({ hosting: { status: 'ready', problem: '', places: up }, routes: { status: 'ready', problem: '', routes: proven } }));
    expect(shown.map(({ value, word, needsYou }) => [value, word, needsYou])).toEqual([
      ['1', 'of 1', false],
      ['2', 'of 2', false],
      ['1', 'of 1', false],
    ]);
  });

  it('shows no count while a section is read, and Not read when the read failed', () => {
    const loading = PLACES.map((place) => ({ ...place, status: 'loading' as const }));
    const shown = figures(reads({ hosting: { status: 'ready', problem: '', places: loading }, routes: { status: 'failed', problem: 'Refused.', routes: [] } }));
    expect(shown.map(({ value, word, needsYou }) => [value, word, needsYou])).toEqual([
      ['2', '1 needs you', true],
      ['–', 'Reading', false],
      ['–', 'Not read', true],
    ]);
  });
});

describe('the lines of what needs you (HSE-019)', () => {
  it('gives each thing that needs you one line with one action, in the order of the numbers and of the machines', () => {
    expect(concerns(reads()).map(({ what, why, action }) => [what, why, action.kind])).toEqual([
      ['mini', 'Down', 'device'],
      ['samba on nas', 'Exited', 'application'],
      ['tracks.example.com', 'Certificate ends in 9 days', 'route'],
      ['wiki.example.com', 'Waits for TXT record', 'route'],
      ['media.example.org', 'Name lost', 'route'],
    ]);
  });

  it('offers to read again a machine whose containers did not answer, and a section that failed', () => {
    const failed = PLACES.map((place) => (place.id === 'm-nas' ? { ...place, status: 'failed' as const, problem: 'SSH refused.', containers: [] } : place));
    const lines = concerns(reads({ hosting: { status: 'ready', problem: '', places: failed }, routes: { status: 'failed', problem: 'Refused.', routes: [] } }));
    expect(lines.map(({ what, why, action }) => [what, why, action])).toEqual([
      ['nas', 'Containers not read: SSH refused.', { kind: 'read', section: 'hosting' }],
      ['mini', 'Down', { kind: 'device', id: 'm-mini' }],
      ['The routes', 'Refused.', { kind: 'read', section: 'routes' }],
    ]);
  });

  it('has no line when nothing needs you', () => {
    const up = PLACES.filter((place) => place.id === 'm-pop');
    expect(concerns(reads({ hosting: { status: 'ready', problem: '', places: up }, routes: { status: 'ready', problem: '', routes: [] } }))).toEqual([]);
  });
});
