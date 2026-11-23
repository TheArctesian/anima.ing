import { fieldErrorsFrom } from '$lib/devices/form/field-errors';
import type { NotificationChannel } from '$lib/notifications/api';
import type { ChannelCalls } from './calls';
import { channelValues, defaultChannelIds, differs, notificationsInput, resendProblem, type ChannelValues, type StoredChannels } from './channels';

export const LOAD_SENTENCE = 'Channels did not load.';
export const REFUSED_SENTENCE = 'The hub refused this interval.';
export const SAVE_SENTENCE = 'Channels were not saved.';
export const ADDED_SENTENCE = 'Check added; channels were not.';
export const PARTIAL_SENTENCE = 'Channels saved; check was not.';

/** `absent`: no hub to ask. */
type ChannelsPhase = 'ready' | 'loading' | 'failed' | 'absent';

type ChannelsSaved = 'sent' | 'skipped' | 'failed';

/** NOT-003, NOT-005: a check's channels go through their own documents (F-3). */
export class ChannelsDraft {
  channels = $state<readonly NotificationChannel[]>([]);
  values = $state<ChannelValues>({ channelIds: [], resend: '0' });
  phase = $state<ChannelsPhase>('absent');
  message = $state('');
  resendInvalid = $state(false);
  /** Retry stays mounted from the first failure until a load succeeds, so its focus survives the reload. */
  retryShown = $state(false);
  readonly #checkId: string | null;
  readonly #calls: ChannelCalls | null;
  #stored: StoredChannels | null = null;
  /** The read in progress, so a save pressed meanwhile waits for it instead of skipping the channels (NOT-003). */
  #loading: Promise<void> = Promise.resolve();
  #savedHere = false;

  constructor(checkId: string | null, calls: ChannelCalls | null) {
    this.#checkId = checkId;
    this.#calls = calls;
    if (calls === null) return;
    this.phase = 'loading';
    void this.load();
  }

  load(): Promise<void> {
    this.#loading = this.#read();
    return this.#loading;
  }

  async #read(): Promise<void> {
    const calls = this.#calls;
    if (calls === null) return;
    this.phase = 'loading';
    this.message = '';
    this.resendInvalid = false;
    try {
      const checkId = this.#checkId;
      const [channels, own] = await Promise.all([calls.channels(), checkId === null ? null : calls.load(checkId)]);
      if (channels.ok && (own === null || own.ok)) {
        this.channels = channels.value;
        if (own !== null && own.ok) {
          this.#stored = own.value;
          this.values = channelValues(own.value);
        } else {
          this.values.channelIds = defaultChannelIds(channels.value);
        }
        this.phase = 'ready';
        this.retryShown = false;
        return;
      }
    } catch {
      // A throwing hub is the same state as one that refused: the sentence below says it.
    }
    this.phase = 'failed';
    this.retryShown = true;
    this.message = LOAD_SENTENCE;
  }

  /** NOT-005: shows the sentence for an unusable resend interval and answers whether the form may save. */
  check(): boolean {
    const problem = this.phase === 'ready' ? resendProblem(this.values) : null;
    this.message = problem ?? '';
    this.resendInvalid = problem !== null;
    return problem === null;
  }

  checkAdded(): void {
    this.message = ADDED_SENTENCE;
    this.resendInvalid = false;
  }

  checkNotSaved(): void {
    if (!this.#savedHere) return;
    this.message = PARTIAL_SENTENCE;
    this.resendInvalid = false;
  }

  async save(checkId: string): Promise<ChannelsSaved> {
    if (this.phase === 'loading') await this.#loading;
    if (this.#calls === null || this.phase !== 'ready') return 'skipped';
    if (this.#stored !== null && !differs(this.#stored, this.values)) return 'skipped';
    try {
      const result = await this.#calls.save(checkId, notificationsInput(this.values));
      if (result.ok) {
        this.#stored = result.value;
        this.#savedHere = true;
        this.message = '';
        return 'sent';
      }
      this.resendInvalid = fieldErrorsFrom(result.error).some((error) => error.field === 'resendEveryBeats');
      this.message = this.resendInvalid ? REFUSED_SENTENCE : SAVE_SENTENCE;
    } catch {
      this.resendInvalid = false;
      this.message = SAVE_SENTENCE;
    }
    return 'failed';
  }
}
