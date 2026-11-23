import type { BuilderShellCode } from '$lib/desktop/builder/shell';
import type { HubError } from '$lib/hub/hub';
import type { BuildJobState, BuildsApi } from './api';

/** The hub takes at most this many lines in one report (`MAX_LINES_PER_REPORT`). */
export const LINES_PER_REPORT = 1000;
export const PENDING_LINES_MAX = 20_000;

/** `lost`: the hub refused the report, so this builder no longer holds the job (BLD-010). */
export type Sent = 'sent' | 'unreachable' | 'lost';

/** A refusal means the lease, the job or the token is gone; anything else may pass on the next try. */
function sentOf(error: HubError): Sent {
  switch (error.code) {
    case 'UNAUTHENTICATED':
    case 'FORBIDDEN':
    case 'NOT_FOUND':
    case 'CONFLICT':
    case 'VALIDATION':
      return 'lost';
    default:
      return 'unreachable';
  }
}

/** BLD-005, BLD-048. */
export class JobReporter {
  #pending: string[] = [];
  /** The number of lines ever queued before `#pending[0]`, sent or dropped. */
  #first = 0;
  #dropped = 0;
  readonly #api: Pick<BuildsApi, 'report'>;
  readonly #jobId: string;

  constructor(api: Pick<BuildsApi, 'report'>, jobId: string) {
    this.#api = api;
    this.#jobId = jobId;
  }

  /** Takes a line that has already been cleaned (BLD-049). */
  add(line: string): void {
    this.#pending.push(line);
    if (this.#pending.length > PENDING_LINES_MAX) {
      this.#pending.shift();
      this.#first += 1;
      this.#dropped += 1;
    }
  }

  #batch(): { lines: string[]; end: number } {
    const note = this.#dropped > 0 ? [`[${String(this.#dropped)} earlier lines were not sent]`] : [];
    const taken = this.#pending.slice(0, LINES_PER_REPORT - note.length);
    return { lines: [...note, ...taken], end: this.#first + taken.length };
  }

  /** The last report carries `state`, so the lines reach the hub while this builder holds the job (BLD-010). */
  async send(state: BuildJobState | null, failureReason: string | null = null): Promise<Sent> {
    const end = this.#first + this.#pending.length;
    for (;;) {
      const batch = this.#batch();
      const last = batch.end >= end;
      const report = { state: last ? state : null, logLines: batch.lines, failureReason: last ? failureReason : null };
      const dropped = this.#dropped;
      const result = await this.#api.report(this.#jobId, report);
      if (!result.ok) return sentOf(result.error);
      this.#pending.splice(0, Math.max(0, batch.end - this.#first));
      this.#first = Math.max(this.#first, batch.end);
      this.#dropped -= Math.min(dropped, this.#dropped);
      if (last) break;
    }
    return 'sent';
  }
}

export type StepCode = BuilderShellCode | 'hub-refused';

const REASONS: Partial<Record<StepCode, string>> = {
  'not-on-main': 'The commit is not on Anima.ing main.',
  dirty: 'Working tree not clean after checkout.',
  'tool-missing': 'A build tool is missing.',
  cancelled: 'Build cancelled on the builder.',
  bundle: 'The bundle is missing after the build.',
  upload: 'A file did not reach the release bucket.',
  'hub-refused': 'The hub refused the upload.',
  busy: 'Another build step was running.',
  commit: 'Builder refused: commit is not a hash.',
  version: 'Builder refused: version is not MAJOR.MINOR.PATCH.',
  target: 'Builder refused: unknown target.',
  platform: 'Builder refused: it cannot build this target.',
};

/** Fixed text, never process output (BLD-010). */
export function failureReason(stepLabel: string, code: StepCode): string {
  if (code === 'step-failed') return `${stepLabel} failed. See the job log.`;
  return REASONS[code] ?? `${stepLabel} could not run.`;
}
