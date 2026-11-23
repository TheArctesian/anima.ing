import { describe, expect, it } from 'vitest';
import { CONTAINERS, MACHINES } from './hosting.fixture';
import { needingYou, placeGroups, type Narrowing, type Place } from './list';

const places: readonly Place[] = MACHINES.map((machine) => ({ ...machine, status: 'ready', problem: '', containers: CONTAINERS[machine.id] ?? [], updates: [] }));
const every: Narrowing = { search: '', machine: '', show: 'all' };
const names = (narrowing: Narrowing, from: readonly Place[] = places) => placeGroups(from, narrowing).map((group) => [group.name, group.rows.map((row) => row.name)]);

describe('placeGroups (HSE-002)', () => {
  it('makes one group for each machine by name, with a Compose project as one row and a lone container as its own row', () => {
    expect(names(every)).toEqual([
      ['mini', []],
      ['nas', ['samba']],
      ['pop-os', ['immich', 'watchtower']],
    ]);
    const immich = placeGroups(places, every)[2]?.rows[0];
    expect(immich?.containers.map((container) => container.name)).toEqual(['immich-redis', 'immich-server']);
    expect(immich?.what).toBe('Compose project');
  });

  it('marks each row Not managed in stage 1: Anima has installed nothing', () => {
    expect(placeGroups(places, every).flatMap((group) => group.rows).every((row) => !row.managed)).toBe(true);
  });

  it('says Nothing installed for a machine that was read and has no container, and gives the running count of the others', () => {
    const [mini, nas] = placeGroups(places, every);
    expect(mini?.empty).toBe(true);
    expect(nas?.empty).toBe(false);
    expect(nas?.summary).toBe('0 of 1 running');
  });

  it('finds a container inside an application on any machine, and hides a machine with no match', () => {
    expect(names({ ...every, search: 'REDIS' })).toEqual([['pop-os', ['immich']]]);
    expect(names({ ...every, search: 'samba' })).toEqual([['nas', ['samba']]]);
  });

  it('narrows the page to one machine', () => {
    expect(names({ ...every, machine: 'm-nas' })).toEqual([['nas', ['samba']]]);
  });

  it('shows only the rows that need the operator under Needs you, and counts them', () => {
    expect(names({ ...every, show: 'needs-you' })).toEqual([['nas', ['samba']]]);
    expect(needingYou(places)).toBe(1);
  });

  it('keeps a machine that did not answer, with the words of the hub', () => {
    const failed = places.map((place) => (place.id === 'm-nas' ? { ...place, status: 'failed' as const, problem: 'No Docker on nas.', containers: [] } : place));
    const nas = placeGroups(failed, every)[1];
    expect(nas).toMatchObject({ name: 'nas', status: 'failed', problem: 'No Docker on nas.', empty: false, rows: [] });
  });

  it('counts the containers with a newer image once the operator asked (HOST-021)', () => {
    const asked = places.map((place) => ({ ...place, updates: [{ containerId: 'a'.repeat(64), container: 'immich-server', image: 'x', state: 'NEWER_IMAGE' as const, reason: null, localDigest: null, registryDigest: null }] }));
    const pop = placeGroups(asked, every)[2];
    expect(pop?.rows.map((row) => row.newer)).toEqual([1, 0]);
    expect(placeGroups(places, every)[2]?.rows[0]?.newer).toBeNull();
  });
});
