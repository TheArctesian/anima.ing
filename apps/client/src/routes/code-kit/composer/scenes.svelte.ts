import { ATTACHMENTS } from '$lib/code/composer/attachments/fixture';
import { CAN, type Can } from '$lib/code/composer/draft';
import type { ComposerEngine, Message, Outcome } from '$lib/code/composer/engine';
import { FIXTURE_SOURCES } from '$lib/code/composer/mentions/fixture';
import { pickerCatalogue, pickerChoice } from '$lib/code/composer/picker/fixture';
import type { Choice } from '$lib/code/composer/picker/catalogue';
import type { Queued } from '$lib/code/composer/queue/queue';

export { ATTACHMENTS };

const OK: Promise<Outcome> = Promise.resolve({ ok: true });

export class SceneEngine implements ComposerEngine {
  phase = $state<'idle' | 'running' | 'ended'>('idle');
  queued = $state<readonly Queued[]>([]);
  choice = $state<Choice>(pickerChoice);
  readonly ended = null;
  readonly attach: Can = CAN;
  readonly dictate: Can = CAN;
  readonly steer: Can = CAN;
  readonly queue: Can = CAN;
  readonly pick: Can = CAN;
  readonly catalogue = pickerCatalogue;
  readonly sources = FIXTURE_SOURCES;
  readonly #fails: string | null;
  readonly queueWaits: string | null;
  #serial = 0;

  constructor(phase: 'idle' | 'running' = 'idle', queued: readonly string[] = [], fails: string | null = null, waits: string | null = null) {
    this.phase = phase;
    this.queued = queued.map((prompt, index) => ({ id: `q${String(index)}`, prompt, saving: false }));
    this.#fails = fails;
    this.queueWaits = waits;
  }

  send = (): Promise<Outcome> => (this.#fails === null ? OK : Promise.resolve({ ok: false, words: this.#fails }));

  interrupt = (): Promise<Outcome> => OK;

  enqueue = (message: Message): Promise<Outcome> => {
    this.#serial += 1;
    this.queued = [...this.queued, { id: `n${String(this.#serial)}`, prompt: message.prompt, saving: false }];
    return OK;
  };

  requeue = (id: string, message: Message): Promise<Outcome> => {
    this.queued = this.queued.map((queued) => (queued.id === id ? { ...queued, prompt: message.prompt } : queued));
    return OK;
  };

  unqueue = (id: string): Promise<Outcome> => {
    this.queued = this.queued.filter((queued) => queued.id !== id);
    return OK;
  };

  sendQueued = (id: string): Promise<Outcome> => this.unqueue(id);

  choose = (next: Choice): Promise<Outcome> => {
    this.choice = next;
    return OK;
  };
}

export const RICH = [
  '# Tighten the gate',
  'Make `quality:limits` **fail** on a file over 300 lines, and *say which one*.',
  '- [x] read [gate.ts](quality/src/gates/gate.ts)',
  '- [ ] run $review on the change',
  '> Keep the baseline shrink-only.',
  '```ts',
  'const LIMIT = 300;',
  '```',
].join('\n');

export const QUEUE = ['Then run the gates of the changed paths.', 'Read [README.md](quality/README.md) and say what is out of date.', 'Open a branch for it.', 'Write the task row.'];
