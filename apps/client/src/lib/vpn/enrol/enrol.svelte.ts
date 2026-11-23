import type { HubError } from '$lib/hub/hub';
import { secondsLeft } from '$lib/metrics/daemon/countdown';
import type { EnrolApi, VpnKey, VpnRegistration } from './api';

export type EnrolCalls = Pick<EnrolApi, 'createInvite' | 'createRouterKey' | 'register'>;

export type EnrolView = 'choose' | 'key' | 'register' | 'registered';

export interface ClipboardSink {
  writeText: (text: string) => Promise<void>;
}

export interface EnrolEnv {
  readonly now: () => Date;
  readonly clipboard: ClipboardSink;
  readonly later: (run: () => void, milliseconds: number) => () => void;
}

const COPIED_MS = 4000;

export interface CopyOutcome {
  readonly text: string;
  readonly ok: boolean;
}

/** VPN-001, VPN-003, VP-7: the pre-auth key lives only in this object and never reaches storage, the URL or a log (LOG-007). */
export class Enrolment {
  view = $state<EnrolView>('choose');
  key = $state<VpnKey | null>(null);
  left = $state(0);
  expired = $state(false);
  busy = $state(false);
  error = $state<HubError | null>(null);
  registered = $state<VpnRegistration | null>(null);
  copied = $state<CopyOutcome | null>(null);

  private expiresAt: string | null = null;
  private epoch = 0;
  private cancelCopied: () => void = () => undefined;

  constructor(
    private readonly api: EnrolCalls,
    private readonly env: EnrolEnv,
  ) {}

  makeRouterKey(siteId: string): Promise<void> {
    return this.makeKey(() => this.api.createRouterKey(siteId));
  }

  makeInvite(): Promise<void> {
    return this.makeKey(() => this.api.createInvite());
  }

  private async makeKey(call: () => ReturnType<EnrolCalls['createInvite']>): Promise<void> {
    if (this.busy) return;
    this.clear();
    const epoch = this.epoch;
    this.busy = true;
    const result = await call();
    if (epoch !== this.epoch) return;
    this.busy = false;
    if (!result.ok) {
      this.error = result.error;
      return;
    }
    this.expiresAt = result.value.expiresAt;
    this.key = result.value;
    this.view = 'key';
    this.tick();
  }

  tick(): void {
    if (this.expiresAt === null) return;
    this.left = secondsLeft(this.expiresAt, this.env.now());
    if (this.left > 0) return;
    this.clear();
    this.expired = true;
    this.view = 'choose';
  }

  /** For a Mac or another computer (vpn.md 6B). */
  openRegister(): void {
    this.clear();
    this.view = 'register';
  }

  async register(authId: string): Promise<void> {
    if (this.busy) return;
    const epoch = this.epoch;
    this.busy = true;
    this.error = null;
    const result = await this.api.register(authId.trim());
    if (epoch !== this.epoch) return;
    this.busy = false;
    if (!result.ok) {
      this.error = result.error;
      return;
    }
    this.registered = result.value;
    this.view = 'registered';
  }

  choose(): void {
    this.clear();
    this.view = 'choose';
  }

  /** The key is one-use, so it leaves the page once a router shows up. */
  keyUsed(): void {
    this.key = null;
    this.expiresAt = null;
    this.left = 0;
    this.forgetCopy();
  }

  close(): void {
    this.clear();
    this.registered = null;
    this.view = 'choose';
  }

  private clear(): void {
    this.epoch += 1;
    this.keyUsed();
    this.expired = false;
    this.busy = false;
    this.error = null;
  }

  async copy(text: string): Promise<void> {
    let ok = true;
    try {
      await this.env.clipboard.writeText(text);
    } catch {
      ok = false;
    }
    this.copied = { text, ok };
    this.cancelCopied();
    this.cancelCopied = this.env.later(() => {
      this.copied = null;
    }, COPIED_MS);
  }

  private forgetCopy(): void {
    this.cancelCopied();
    this.copied = null;
  }
}
