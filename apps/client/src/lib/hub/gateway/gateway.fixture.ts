// Answers as `anima-cloud door` does to a request that is not a person opening a page.

export interface GatewayRequest {
  readonly path: string;
  readonly method: string;
}

function refusal(status: number, code: string): Response {
  return new Response(JSON.stringify({ error: { code, message: 'words of the gateway' } }), { status });
}

export function fakeGateway(state: { session: boolean; hub?: () => Response }) {
  const requests: GatewayRequest[] = [];
  const send: typeof fetch = (input, init) => {
    const path = new URL(String(input)).pathname;
    const method = init?.method ?? 'GET';
    requests.push({ path, method });
    if (path === '/_door/sign-out' && method === 'POST') {
      state.session = false;
      // What a browser gives a page for a redirect that it was told not to follow.
      return Promise.resolve(Object.defineProperty(new Response(null), 'type', { value: 'opaqueredirect' }));
    }
    if (path.startsWith('/_door/')) return Promise.resolve(refusal(404, 'no_such_route'));
    if (!state.session) return Promise.resolve(refusal(401, 'sign_in_needed'));
    return Promise.resolve(state.hub?.() ?? refusal(503, 'hub_not_ready'));
  };
  return { send, requests, state };
}

export const hubAlone: typeof fetch = () =>
  Promise.resolve(new Response('<!doctype html><title>Anima.ing</title>', { status: 200, headers: { 'content-type': 'text/html' } }));
