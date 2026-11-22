import { afterEach, expect, it } from 'vitest';
import { createHub, NO_HUB_ADDRESS, type Hub } from '$lib/hub/hub';
import { FAKE_WEBSOCKET } from '$lib/hub/socket/fake';
import { routesApi } from '../api';

interface Sent {
  readonly query: string;
  readonly variables: Record<string, unknown>;
}

const hubs: Hub[] = [];

afterEach(async () => {
  await Promise.all(hubs.splice(0).map((hub) => hub.close()));
});

/** The route calls on a real hub client whose HTTP endpoint answers with `body`, recording what was sent. */
function routesAnswering(body: unknown) {
  const sent: Sent[] = [];
  const fakeFetch: typeof fetch = (_input, init) => {
    sent.push(JSON.parse(String(init?.body)) as Sent);
    return Promise.resolve(new Response(JSON.stringify(body)));
  };
  const hub = createHub({ address: NO_HUB_ADDRESS, token: () => Promise.resolve('operator-token'), fetch: fakeFetch, WebSocket: FAKE_WEBSOCKET });
  hubs.push(hub);
  return { api: routesApi(hub), sent };
}

it('H21: remove sends only the ID of the route and returns the removed ID', async () => {
  const { api, sent } = routesAnswering({ data: { removeApplicationRoute: 'route-vault' } });
  expect(await api.remove('route-vault')).toEqual({ ok: true, value: 'route-vault' });
  expect(sent).toHaveLength(1);
  expect(sent[0]?.query).toContain('removeApplicationRoute(id: $id)');
  expect(sent[0]?.variables).toEqual({ id: 'route-vault' });
});

it('H21: a refused removal comes back as the refusal of the hub', async () => {
  const { api } = routesAnswering({ data: null, errors: [{ message: 'No such route.', extensions: { code: 'NOT_FOUND' } }] });
  expect(await api.remove('route-gone')).toEqual({ ok: false, error: { code: 'NOT_FOUND', message: 'No such route.' } });
});
