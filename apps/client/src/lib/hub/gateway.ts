export interface GatewayAnswer {
  readonly status: number;
  readonly body: unknown;
}

/** A path the pool's gateway answers itself, never the hub (HMD-073); the caller checks the answer, a failure gives no text (LOG-007). */
export async function readGateway(path: string, waitMs: number, fetcher: typeof fetch = fetch): Promise<GatewayAnswer | null> {
  try {
    const response = await fetcher(path, { credentials: 'same-origin', signal: AbortSignal.timeout(waitMs) });
    const body: unknown = response.ok ? await response.json() : null;
    return { status: response.status, body };
  } catch {
    return null;
  }
}
