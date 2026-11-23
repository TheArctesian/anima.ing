import { expect, it } from 'vitest';
import type { Hub, HubResult, Variables } from '$lib/hub/hub';
import { hostingApi } from './api';

const node = (id: string, enabled: boolean) => ({ node: { id, name: id, status: 'UP', ssh: { enabled } } });

/** Two pages of devices; the second is asked with the cursor of the first. */
const PAGES: Readonly<Record<string, unknown>> = {
  '': { devices: { pageInfo: { hasNextPage: true, endCursor: 'c1' }, edges: [node('atlas', true), node('phone', false)] } },
  c1: { devices: { pageInfo: { hasNextPage: false, endCursor: null }, edges: [node('nas', true)] } },
};

function pagedHub(asked: (string | null)[]): Pick<Hub, 'request'> {
  const request = (_document: unknown, variables: Variables): Promise<HubResult<unknown>> => {
    const after = (variables as { after: string | null }).after;
    asked.push(after);
    return Promise.resolve({ ok: true, value: PAGES[after ?? ''] });
  };
  return { request: request as Hub['request'] };
}

it('HSE-002: the machines are the devices with SSH on, read over all the pages of the hub', async () => {
  const asked: (string | null)[] = [];
  const result = await hostingApi(pagedHub(asked)).machines();
  expect(result).toEqual({ ok: true, value: [{ id: 'atlas', name: 'atlas', state: 'UP' }, { id: 'nas', name: 'nas', state: 'UP' }] });
  expect(asked).toEqual([null, 'c1']);
});

it('HSE-002: a refusal of the hub is the answer, with its words', async () => {
  const refused: Pick<Hub, 'request'> = { request: (() => Promise.resolve({ ok: false, error: { code: 'FORBIDDEN', message: 'No.' } })) as Hub['request'] };
  expect(await hostingApi(refused).machines()).toEqual({ ok: false, error: { code: 'FORBIDDEN', message: 'No.' } });
});
