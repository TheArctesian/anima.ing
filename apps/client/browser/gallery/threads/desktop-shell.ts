import type { BrowserContext } from '@playwright/test';
import { BEARER_TOKEN, HUB_ORIGIN } from '../../synthetic/access';

/**
 * A stand-in for the desktop shell, so that the gallery can photograph the page Threads, which only the desktop
 * application shows (THR-011). It answers the commands of the shell from a scenario of made-up threads; the app's own
 * code reads them as it reads the shell. It reaches no runner, no session file and no key store of this computer.
 */

/** One state of the threads of a computer. A string `@<minutes>` is a time that many minutes from the shot (`@-10`). */
export interface Scenario {
  /** The session files, as `agent_threads_list` answers. */
  readonly rows: readonly Record<string, unknown>[];
  /** Each session file, as `agent_thread_read` answers, by ID. */
  readonly threads: Readonly<Record<string, Record<string, unknown>>>;
  /** The threads that the runner drives, as the call `list` answers. */
  readonly running: readonly Record<string, unknown>[];
  /** Words while the holder of the programs is older than the daemon (TH98). */
  readonly holder?: string;
  /** How the person lists each thread (TH46). */
  readonly listings: readonly Record<string, unknown>[];
  /** The updates the watch sends when it starts, each with its thread and sequence number. */
  readonly updates: readonly Record<string, unknown>[];
  /** The runner of this computer is not running: the watch and each call fail. */
  readonly runnerDown?: true;
  /** What the project files of each folder run by themselves (THR-046), by folder. */
  readonly notices?: Readonly<Record<string, readonly string[]>>;
  /** The answer to a start: a refusal, for the dialog. */
  readonly startAnswer?: Record<string, unknown>;
  /** The models each program lists, as the call `models` answers (TH104); none when left out. */
  readonly models?: readonly Record<string, unknown>[];
  /** The program's words when `models` fails, as a runner that could not ask it answers. */
  readonly modelsSaid?: string;
}

/** What the first script leaves on the window for the second: the scenario with its times, and the runner's answers. */
interface Gallery {
  readonly world: Scenario;
  readonly runner: (action: Record<string, unknown>) => unknown;
}

/** The scenario with real times, and the answer of the runner to each call. Runs in the page. */
function seedRunner(made: Scenario): void {
  const now = Date.now();
  const timed = (value: unknown): unknown => {
    if (typeof value === 'string' && value.startsWith('@')) return new Date(now + Number(value.slice(1)) * 60_000).toISOString();
    if (Array.isArray(value)) return value.map(timed);
    if (typeof value === 'object' && value !== null) return Object.fromEntries(Object.entries(value).map(([key, each]) => [key, timed(each)]));
    return value;
  };
  const world = timed(made) as Scenario;
  const answers: Record<string, (action: Record<string, unknown>) => unknown> = {
    list: () => (world.holder === undefined ? { outcome: 'threads', threads: world.running } : { outcome: 'threads', threads: world.running, holder: world.holder }),
    listings: () => ({ outcome: 'listings', listings: world.listings }),
    change_listing: () => ({ outcome: 'listings', listings: world.listings }),
    folder: (action) => ({ outcome: 'folder', notice: world.notices?.[String(action['folder'])] ?? [] }),
    models: () =>
      world.modelsSaid === undefined ? { outcome: 'models', models: world.models ?? [] } : { outcome: 'refused', refusal: 'program_does_not_answer', said: world.modelsSaid },
    start: () => world.startAnswer ?? { outcome: 'started', thread: String(world.running[0]?.['id'] ?? '') },
  };
  const gallery: Gallery = { world, runner: (action) => answers[String(action['call'])]?.(action) ?? { outcome: 'ok' } };
  Reflect.set(window, '__gallery', gallery);
}

/** The commands of the shell, answered from the scenario; the app takes itself for the desktop application. Runs in the page. */
function standIn({ origin, token }: { origin: string; token: string }): void {
  const { world, runner } = Reflect.get(window, '__gallery') as Gallery;
  const handlers = new Map<number, (message: unknown) => void>();
  const listeners = new Map<string, number>();
  let next = 1;
  const emit = (event: string, payload: unknown): void => {
    const id = listeners.get(event);
    if (id !== undefined) handlers.get(id)?.({ event, id, payload });
  };
  const fail = (code: string): Promise<never> => Promise.reject(Object.assign(new Error(code), { code }));
  const watch = (): Promise<unknown> => {
    if (world.runnerDown === true) return fail('not_running');
    setTimeout(() => {
      emit('agent-runner:link', true);
      for (const update of world.updates) emit('agent-runner:update', update);
    }, 50);
    return Promise.resolve(null);
  };
  const commands: Record<string, (args: Record<string, unknown>) => Promise<unknown>> = {
    access_token_read: () => Promise.resolve(token),
    agent_threads_list: () => Promise.resolve(world.rows),
    agent_thread_read: (args) => {
      const thread = world.threads[String(args['id'])];
      return thread === undefined ? fail('not_found') : Promise.resolve(thread);
    },
    agent_runner_call: (args) => (world.runnerDown === true ? fail('not_running') : Promise.resolve(runner((args['action'] ?? {}) as Record<string, unknown>))),
    agent_runner_watch: watch,
    'plugin:event|listen': (args) => {
      const id = Number(args['handler']);
      listeners.set(String(args['event']), id);
      return Promise.resolve(id);
    },
  };
  const quiet = new Set(['plugin:event|unlisten', 'plugin:updater|check', 'plugin:log|log']);
  localStorage.setItem('anima.hubAddress', origin);
  Reflect.set(window, 'isTauri', true);
  Reflect.set(window, '__TAURI_INTERNALS__', {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback: (handler: (message: unknown) => void) => {
      const id = next++;
      handlers.set(id, handler);
      return id;
    },
    unregisterCallback: (id: number) => handlers.delete(id),
    convertFileSrc: (path: string) => path,
    invoke: (command: string, args: Record<string, unknown> | undefined) => {
      if (quiet.has(command)) return Promise.resolve(null);
      return commands[command]?.(args ?? {}) ?? fail('failed');
    },
  });
}

/** Installs the stand-in before any script of the app runs. */
export async function installDesktopShell(context: BrowserContext, scenario: Scenario): Promise<void> {
  await context.addInitScript(seedRunner, scenario);
  await context.addInitScript(standIn, { origin: HUB_ORIGIN, token: BEARER_TOKEN });
}
