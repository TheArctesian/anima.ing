import { createContext } from 'svelte';
import type { BuilderShell } from '$lib/desktop/builder/shell';
import type { HubResult } from '$lib/hub/hub';
import type { BuilderPlatform, BuildsApi } from './api';
import { startBuilderLoop, type BuilderStatus } from './loop';

/** Where this computer keeps its builder token: the OS key store, through the shell (BLD-023). */
export interface BuilderTokenStore {
  read(): Promise<string | null>;
  write(token: string): Promise<void>;
  clear(): Promise<void>;
}

/** Where this computer keeps its builder ID, which is not secret, so that turning building off can revoke it (BLD-025). */
export interface BuilderIdStore {
  read(): string | null;
  write(id: string): void;
  clear(): void;
}

export interface ThisBuilderPorts {
  /** `enrol` and `revoke` with the operator's session; `claim` and `report` with the builder token. */
  readonly operator: Pick<BuildsApi, 'enrol' | 'revoke'>;
  readonly builder: Pick<BuildsApi, 'claim' | 'report' | 'uploadUrls' | 'reportUploaded'>;
  readonly shell: BuilderShell;
  readonly tokens: BuilderTokenStore;
  readonly ids: BuilderIdStore;
  /** The operator's access token, removed from every log line like the builder token (BLD-049). */
  readonly operatorToken: () => Promise<string | null>;
  readonly computerName: () => Promise<string>;
  readonly sleep: (ms: number) => Promise<void>;
  readonly now: () => Date;
}

const PROBLEMS = {
  enrol: 'The hub did not enrol this computer.',
  store: 'Builder token not saved to the key store.',
  revoke: 'Not revoked. Revoke it in the list below.',
  unknownId: 'Not revoked here. Revoke it in the list below.',
  clear: 'Builder token not deleted from the key store. Turn building off again.',
  read: 'Builder token not read from the key store.',
  tools: 'Platform not detected.',
} as const;

/** This computer as a builder (BLD-020 to BLD-029). */
export class ThisBuilder {
  /** `null` until the key store has been read. */
  enrolled = $state<boolean | null>(null);
  status = $state<BuilderStatus>({ kind: 'stopped' });
  problem = $state<string | null>(null);
  busy = $state(false);

  readonly #ports: ThisBuilderPorts;
  #token: string | null = null;
  #operatorToken: string | null = null;
  #stop: (() => Promise<void>) | null = null;

  constructor(ports: ThisBuilderPorts) {
    this.#ports = ports;
  }

  /** The operator's token, or `null` when the key store cannot say; a log line is then cleaned of the builder token only. */
  async #readOperatorToken(): Promise<string | null> {
    try {
      return await this.#ports.operatorToken();
    } catch {
      return null;
    }
  }

  #secrets(): readonly string[] {
    return [this.#token, this.#operatorToken].filter((secret) => secret !== null);
  }

  #start(token: string, operatorToken: string | null): void {
    this.#token = token;
    this.#operatorToken = operatorToken;
    this.#stop = startBuilderLoop({
      api: this.#ports.builder,
      shell: this.#ports.shell,
      secrets: () => this.#secrets(),
      sleep: this.#ports.sleep,
      now: this.#ports.now,
      onStatus: (status) => {
        this.status = status;
      },
    });
  }

  /** The builder runs while the app runs (BLD-026). */
  async resume(): Promise<void> {
    try {
      const token = await this.#ports.tokens.read();
      this.enrolled = token !== null;
      if (token === null) return;
      const operatorToken = await this.#readOperatorToken();
      if (this.#stop === null) this.#start(token, operatorToken);
    } catch {
      this.enrolled = false;
      this.problem = PROBLEMS.read;
    }
  }

  /** A failure's reason may name the key store, so only `problem` shows (LOG-007). */
  async #attempt(problem: string, call: () => Promise<unknown>): Promise<boolean> {
    try {
      await call();
      return true;
    } catch {
      this.problem = problem;
      return false;
    }
  }

  async #platform(): Promise<BuilderPlatform | null> {
    try {
      const { os, arch } = await this.#ports.shell.checkTools();
      return { os, arch };
    } catch {
      this.problem = PROBLEMS.tools;
      return null;
    }
  }

  async enable(): Promise<void> {
    if (this.busy || this.enrolled === true) return;
    this.busy = true;
    this.problem = null;
    try {
      const platform = await this.#platform();
      if (platform === null) return;
      const enrolment = await this.#ports.operator.enrol(await this.#ports.computerName(), platform);
      if (!enrolment.ok) {
        this.problem = PROBLEMS.enrol;
        return;
      }
      const { builderId, token } = enrolment.value;
      if (!(await this.#attempt(PROBLEMS.store, () => this.#ports.tokens.write(token)))) {
        await this.#ports.operator.revoke(builderId);
        return;
      }
      this.#ports.ids.write(builderId);
      const operatorToken = await this.#readOperatorToken();
      this.enrolled = true;
      this.#start(token, operatorToken);
    } finally {
      this.busy = false;
    }
  }

  /** When the key store keeps the token, the ID and the enrolled state stay, so turning building off again can finish. */
  async #forgetHere(): Promise<void> {
    await this.stop();
    if (!(await this.#attempt(PROBLEMS.clear, () => this.#ports.tokens.clear()))) return;
    this.#ports.ids.clear();
    this.#token = null;
    this.#operatorToken = null;
    this.enrolled = false;
    this.status = { kind: 'stopped' };
  }

  async disable(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    this.problem = null;
    try {
      await this.stop();
      const id = this.#ports.ids.read();
      const revoked: HubResult<string> | null = id === null ? null : await this.#ports.operator.revoke(id);
      if (revoked === null) this.problem = PROBLEMS.unknownId;
      else if (!revoked.ok && revoked.error.code !== 'NOT_FOUND') this.problem = PROBLEMS.revoke;
      await this.#forgetHere();
    } finally {
      this.busy = false;
    }
  }

  async revoked(id: string): Promise<void> {
    if (this.#ports.ids.read() === id) await this.#forgetHere();
  }

  /** The token stays. */
  async stop(): Promise<void> {
    const stop = this.#stop;
    this.#stop = null;
    if (stop !== null) await stop();
  }
}

const [getThisBuilder, setThisBuilder] = createContext<ThisBuilder>();

export function provideThisBuilder(builder: ThisBuilder): void {
  setThisBuilder(builder);
}

/** The builder set by `provideThisBuilder`; throws when there is none. */
export function useThisBuilder(): ThisBuilder {
  return getThisBuilder();
}
