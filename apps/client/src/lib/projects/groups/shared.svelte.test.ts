// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it } from 'vitest';
import { createHub, NO_HUB_ADDRESS, type Hub } from '$lib/hub/hub';
import { FAKE_WEBSOCKET } from '$lib/hub/socket/fake';
import { storedClusters, storeClusters, type Clusters } from './clusters';
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

it.each([
  'Unknown type "ProjectGroupImportInput"',
  'Unknown field "importProjectGroup" on type "Mutation".',
  'Invalid value for argument "input", unknown field "includeEmpty" of type "ProjectGroupImportInput"',
])('G3: unsupported import preserves device groups, keeps shared groups usable and pauses automatic retries: %s', async (message) => {
  const empty = { id: 'empty', name: 'Empty' };
  const kept = { groups: message.includes('includeEmpty') ? [empty, ...local.groups] : [...local.groups, empty], of: local.of };
  storeClusters(localStorage, kept);
  let imports = 0;
  let supported = false;
  const answer: typeof fetch = (_url, init) => {
    const request = JSON.parse(String(init?.body)) as { variables: { input?: { projects: string[] } } };
    if (request.variables.input === undefined) return Promise.resolve(response({ projectGroups: [shared] }));
    imports += 1;
    return Promise.resolve(supported
      ? response({ importProjectGroup: request.variables.input.projects.filter((id) => id === 'p1') })
      : new Response(JSON.stringify({ errors: [{ message }] })));
  };
  const groups = client(answer);
  await groups.refresh();
  expect(groups.problem).toBe('');
  expect(groups.importUnserved).toBe(true);
  expect(groups.unserved).toBe(false);
  expect(groups.ready).toBe(true);
  expect(groups.of('p1')?.name).toBe('Work');
  await groups.refresh();
  await groups.refresh();
  expect(imports).toBe(1);
  expect(storedClusters(localStorage)).toEqual(kept);
  supported = true;
  const reopened = client(answer);
  await reopened.refresh();
  expect(reopened.importUnserved).toBe(false);
  expect(storedClusters(localStorage)).toEqual({ ...local, of: { foreign: source } });
});

it.each(['FORBIDDEN', 'UNAUTHENTICATED', 'INTERNAL'])('G3: a coded %s import failure stays an error and can retry', async (code) => {
  storeClusters(localStorage, local);
  const message = 'Unknown type "ProjectGroupImportInput"';
  let imports = 0;
  const groups = client((_url, init) => {
    const request = JSON.parse(String(init?.body)) as { variables: { input?: unknown } };
    if (request.variables.input === undefined) return Promise.resolve(response({ projectGroups: [shared] }));
    imports += 1;
    return Promise.resolve(new Response(JSON.stringify({ errors: [{ message, extensions: { code } }] })));
  });
  await groups.refresh();
  expect(groups.importUnserved).toBe(false);
  expect(groups.problem).toBe(message);
  await groups.refresh();
  expect(imports).toBe(2);
  expect(storedClusters(localStorage)).toEqual(local);
});

it('G3: groups over the hub limit use batches', async () => {
  const ids = Array.from({ length: 201 }, (_, i) => `p${String(i)}`);
  storeClusters(localStorage, { groups: local.groups, of: Object.fromEntries(ids.map((id) => [id, source])) });
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
  expect(storedClusters(localStorage)).toEqual({ groups: [], of: {} });
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


it('G3: an empty legacy group imports into the next hub and refreshes despite no acknowledged project IDs', async () => {
  storeClusters(localStorage, { groups: local.groups, of: {} });
  const sent: unknown[] = [];
  const answers = [response({ projectGroups: [] }), response({ importProjectGroup: [] }), response({ projectGroups: [{ ...shared, projectIds: [] }] })];
  const groups = client((_url, init) => {
    const request = JSON.parse(String(init?.body)) as { variables: { input?: unknown } };
    if (request.variables.input !== undefined) sent.push(request.variables.input);
    return Promise.resolve(answers.shift() ?? response({ projectGroups: [] }));
  });
  await groups.refresh();
  expect(groups.groups).toEqual([{ id: shared.id, name: 'Work' }]);
  expect(storedClusters(localStorage)).toEqual({ groups: [], of: {} });
  expect(sent).toEqual([{ source, name: 'Work', projects: [], includeEmpty: true }]);
});

it('G3: an empty group stays local after a network failure and retries safely', async () => {
  storeClusters(localStorage, { groups: local.groups, of: {} });
  const failed = new Response('', { status: 503 });
  const answers = [response({ projectGroups: [] }), failed, response({ projectGroups: [] }), response({ importProjectGroup: [] }), response({ projectGroups: [{ ...shared, projectIds: [] }] })];
  const groups = client(() => Promise.resolve(answers.shift() ?? response({ projectGroups: [] })));
  await groups.refresh();
  expect(storedClusters(localStorage)).toEqual({ groups: local.groups, of: {} });
  expect(groups.problem).not.toBe('');
  await groups.refresh();
  expect(storedClusters(localStorage)).toEqual({ groups: [], of: {} });
});

it.each(['member', 'name'] as const)('G3: empty-group acknowledgement preserves a concurrent local %s change', async (change) => {
  storeClusters(localStorage, { groups: local.groups, of: {} });
  const updated: Clusters = change === 'member' ? { groups: local.groups, of: { added: source } } : { groups: [{ id: source, name: 'Renamed' }], of: {} };
  let calls = 0;
  const groups = client(() => {
    calls += 1;
    if (calls === 2) {
      storeClusters(localStorage, updated);
      return Promise.resolve(response({ importProjectGroup: [] }));
    }
    return Promise.resolve(response({ projectGroups: [] }));
  });
  await groups.refresh();
  expect(calls).toBe(3);
  expect(storedClusters(localStorage)).toEqual(updated);
});
