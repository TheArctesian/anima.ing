// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it } from 'vitest';
import { createHub, NO_HUB_ADDRESS, type Hub } from '$lib/hub/hub';
import { FAKE_WEBSOCKET } from '$lib/hub/socket/fake';
import { storedClusters, storeClusters } from './clusters';
import { SharedProjectGroups } from './shared.svelte';

const hubs: Hub[] = [];
const source = '0190a000-0000-7000-9003-000000000001';
const local = { groups: [{ id: source, name: 'Work' }], of: { p1: source, foreign: source } };
const shared = { id: 'shared', name: 'Work', projectIds: ['p1'] };
const response = (data: unknown) => new Response(JSON.stringify({ data }));

beforeEach(() => localStorage.clear());
afterEach(async () => { await Promise.all(hubs.splice(0).map((hub) => hub.close())); });

function client(answer: typeof fetch) {
  const hub = createHub({ address: NO_HUB_ADDRESS, token: () => Promise.resolve('test-token'), fetch: answer, WebSocket: FAKE_WEBSOCKET });
  hubs.push(hub);
  return new SharedProjectGroups(hub, localStorage);
}

it('G3: opening Projects imports local groups without an action and keeps unacknowledged projects', async () => {
  storeClusters(localStorage, local);
  const answers = [response({ projectGroups: [] }), response({ importProjectGroup: ['p1'] }), response({ projectGroups: [shared] })];
  const groups = client(() => Promise.resolve(answers.shift() ?? response({ projectGroups: [shared] })));
  await groups.refresh();
  expect(groups.of('p1')?.name).toBe('Work');
  expect(storedClusters(localStorage)).toEqual({ ...local, of: { foreign: source } });
});

it('G3: a lost reply keeps local data, then a successful retry retires only acknowledged memberships', async () => {
  storeClusters(localStorage, { ...local, of: { p1: source } });
  const answers = [response({ projectGroups: [] }), new Response('', { status: 503 }), response({ projectGroups: [shared] }), response({ importProjectGroup: ['p1'] }), response({ projectGroups: [shared] })];
  const groups = client(() => Promise.resolve(answers.shift() ?? response({ projectGroups: [shared] })));
  await groups.refresh();
  expect(storedClusters(localStorage).of).toEqual({ p1: source });
  expect(groups.problem).not.toBe('');
  await groups.refresh();
  expect(storedClusters(localStorage)).toEqual({ groups: [], of: {} });
  expect(groups.of('p1')?.name).toBe('Work');
  expect(groups.problem).toBe('');
});

it('G3: an acknowledgement preserves another tab’s new group, membership and changed assignment', async () => {
  storeClusters(localStorage, local);
  let calls = 0;
  const groups = client(() => {
    calls += 1;
    if (calls === 2) {
      storeClusters(localStorage, {
        groups: [...local.groups, { id: 'other', name: 'Other' }],
        of: { p1: 'other', foreign: source, added: source, elsewhere: 'other' },
      });
      return Promise.resolve(response({ importProjectGroup: ['p1'] }));
    }
    return Promise.resolve(response({ projectGroups: [shared] }));
  });
  await groups.refresh();
  expect(storedClusters(localStorage)).toEqual({
    groups: [...local.groups, { id: 'other', name: 'Other' }],
    of: { p1: 'other', foreign: source, added: source, elsewhere: 'other' },
  });
});

it('G3: an older hub keeps local groups and leaves existing shared groups usable', async () => {
  storeClusters(localStorage, local);
  let calls = 0;
  const groups = client(() => {
    calls += 1;
    return Promise.resolve(calls === 1 ? response({ projectGroups: [shared] }) : new Response(JSON.stringify({ errors: [{ message: 'Unknown field "importProjectGroup" on type "Mutation".' }] })));
  });
  await groups.refresh();
  expect(storedClusters(localStorage)).toEqual(local);
  expect(groups.of('p1')?.name).toBe('Work');
  expect(groups.unserved).toBe(false);
  expect(groups.problem).toContain('Sharing earlier groups needs a newer hub');
});

it('G3: groups over the hub limit use batches and no empty group is sent', async () => {
  const ids = Array.from({ length: 201 }, (_, i) => `p${String(i)}`);
  storeClusters(localStorage, { groups: [...local.groups, { id: 'empty', name: 'Empty' }], of: Object.fromEntries(ids.map((id) => [id, source])) });
  const sizes: number[] = [];
  const groups = client((_url, init) => {
    const request = JSON.parse(String(init?.body)) as { variables: { input?: { projects: string[] } } };
    const projects = request.variables.input?.projects;
    if (projects === undefined) return Promise.resolve(response({ projectGroups: [] }));
    sizes.push(projects.length);
    return Promise.resolve(response({ importProjectGroup: projects }));
  });
  await groups.refresh();
  expect(sizes).toEqual([200, 1]);
  expect(storedClusters(localStorage)).toEqual({ groups: [{ id: 'empty', name: 'Empty' }], of: {} });
});


it('G3: one rejected legacy name does not block later valid groups', async () => {
  const bad = { id: 'invalid', name: 'Bad\nname' };
  storeClusters(localStorage, { groups: [bad, ...local.groups], of: { p2: bad.id, p1: source } });
  const answers = [
    response({ projectGroups: [] }),
    new Response(JSON.stringify({ errors: [{ message: 'Give a name with no control character.', extensions: { code: 'VALIDATION', fields: [] } }] })),
    response({ importProjectGroup: ['p1'] }),
    response({ projectGroups: [shared] }),
  ];
  const groups = client(() => Promise.resolve(answers.shift() ?? response({ projectGroups: [shared] })));
  await groups.refresh();
  expect(groups.of('p1')?.name).toBe('Work');
  expect(groups.problem).toContain('control character');
  expect(storedClusters(localStorage)).toEqual({ groups: [bad], of: { p2: bad.id } });
});
