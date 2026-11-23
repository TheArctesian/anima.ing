import { cloudHubUsage } from '$lib/desktop/cloud/shell';
import { readGateway } from '$lib/hub/hub';
import { capabilities } from '$lib/platform/platform';
import { parseUsage, type UsageAnswer } from './usage';

export const GATEWAY_PATH = '/_door/usage';
export const WAIT_MS = 10_000;

const SIGNED_OUT: UsageAnswer = { state: 'signed-out' };
const UNAVAILABLE: UsageAnswer = { state: 'unavailable' };

function figures(value: unknown, hubId?: string): UsageAnswer {
  const usage = parseUsage(value, hubId);
  return usage === undefined ? UNAVAILABLE : { state: 'figures', usage };
}

async function fromShell(): Promise<UsageAnswer> {
  const answered = await cloudHubUsage();
  if (answered.ok) return figures(answered.value);
  return answered.problem === 'signed-out' ? SIGNED_OUT : UNAVAILABLE;
}

/** The hub id is the first label of the page's host (`https://<hub id>.hub.anima.ing`, AC-D45). */
function hubIdOfPage(): string | undefined {
  return typeof location === 'undefined' ? undefined : location.hostname.split('.')[0];
}

async function fromGateway(fetcher: typeof fetch | undefined): Promise<UsageAnswer> {
  const answered = await readGateway(GATEWAY_PATH, WAIT_MS, fetcher);
  if (answered?.status === 401) return SIGNED_OUT;
  return answered !== null && answered.status === 200 ? figures(answered.body, hubIdOfPage()) : UNAVAILABLE;
}

/** The figures come from the account, never from the hub (HMD-073); a failure is a state of the page, never its text. */
export function readAccountUsage(inShell: boolean = capabilities().cloudAccount, fetcher?: typeof fetch): Promise<UsageAnswer> {
  return inShell ? fromShell() : fromGateway(fetcher);
}
