import type { HubError, HubResult } from '$lib/hub/hub';
import type { DaemonEnrolment, DaemonStatusView, metricsApi } from '$lib/metrics/api';
import { clearTime, secondsLeft } from './countdown';

export type DaemonCalls = Pick<ReturnType<typeof metricsApi>, 'daemonStatus' | 'enrolDaemon' | 'revokeDaemonToken'>;

export interface ClipboardSink {
  writeText: (text: string) => Promise<void>;
}

export interface PanelEnv {
  readonly now: () => Date;
  readonly clipboard: ClipboardSink;
  readonly later: (run: () => void, milliseconds: number) => () => void;
}

const COPIED_MS = 4000;

/** `count` makes each copy a new announcement. */
export interface CopyOutcome {
  readonly text: string;
  readonly ok: boolean;
  readonly count: number;
}

/** MET-018, MET-019: the enrolment code lives only in this object and never reaches storage, the URL or a log (LOG-007). */
export class DaemonInstall {
  code = $state<string | null>(null);
  left = $state(0);
  expired = $state(false);
  busy = $state(false);
  error = $state<HubError | null>(null);
  copied = $state<CopyOutcome | null>(null);
  status = $state<DaemonStatusView | null>(null);
  statusError = $state<HubError | null>(null);
  /** The source stays DAEMON, stale, until it enrols again (R-092 D-7). */
  revoked = $state(false);

  private expiresAt: string | null = null;
  private epoch = 0;
  private statusEpoch = 0;
  private copies = 0;
  private cancelCopied: () => void = () => undefined;

  constructor(
    private readonly calls: DaemonCalls,
    private readonly deviceId: string,
    private readonly env: PanelEnv,
  ) {}

  /** Asks the hub for a fresh code (MET-012). */
  async makeCode(): Promise<void> {
    // A second code voids the first (K8 Section 3), so a request in flight is never doubled.
    if (this.busy) return;
    this.clear();
    const epoch = this.epoch;
    this.busy = true;
    const result = await this.calls.enrolDaemon(this.deviceId);
    if (epoch !== this.epoch) return;
    this.busy = false;
    if (!result.ok) {
      this.error = result.error;
      return;
    }
    this.revoked = false;
    this.show(result.value);
  }

  private show({ code, expiresAt }: DaemonEnrolment): void {
    this.expiresAt = clearTime(expiresAt, this.env.now());
    this.code = code;
    this.tick();
  }

  tick(): void {
    if (this.expiresAt === null) return;
    this.left = secondsLeft(this.expiresAt, this.env.now());
    if (this.left > 0) return;
    this.clear();
    this.expired = true;
    void this.loadStatus();
  }

  /** The code may have been used, so the daemon state is read again. */
  close(): void {
    this.clear();
    void this.loadStatus();
  }

  clear(): void {
    this.epoch += 1;
    this.code = null;
    this.expiresAt = null;
    this.left = 0;
    this.expired = false;
    this.busy = false;
    this.error = null;
    this.forgetCopy();
  }

  async copy(text: string): Promise<void> {
    let ok = true;
    try {
      await this.env.clipboard.writeText(text);
    } catch {
      ok = false;
    }
    this.copies += 1;
    this.copied = { text, ok, count: this.copies };
    this.cancelCopied();
    this.cancelCopied = this.env.later(() => {
      this.copied = null;
    }, COPIED_MS);
  }

  private forgetCopy(): void {
    this.cancelCopied();
    this.copied = null;
  }

  async loadStatus(): Promise<void> {
    this.statusEpoch += 1;
    const epoch = this.statusEpoch;
    const result = await this.calls.daemonStatus(this.deviceId);
    if (epoch !== this.statusEpoch) return;
    this.statusError = result.ok ? null : result.error;
    this.status = result.ok ? result.value : null;
    if (result.ok && result.value?.metricsStale === false) this.revoked = false;
  }

  /** Revokes the token once the operator confirmed. */
  async revoke(): Promise<HubResult<DaemonStatusView>> {
    const result = await this.calls.revokeDaemonToken(this.deviceId);
    if (result.ok) {
      this.statusEpoch += 1;
      this.status = result.value;
      this.statusError = null;
      this.revoked = true;
    }
    return result;
  }
}
