import { SvelteMap } from 'svelte/reactivity';
import type { ThreadEntry } from './api';

export interface Message {
  readonly sequence: string;
  readonly role: 'you' | 'agent' | 'notice';
  readonly text: string;
}

export interface PendingRequest {
  readonly kind: string;
  readonly questions: readonly { key: string; text: string; choices: string[]; many: boolean }[];
  readonly id: string;
  readonly title: string;
  readonly subject: string;
  readonly diff: string;
}

function record(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {};
}

function text(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function requestTitle(kind: unknown): string {
  switch (kind) {
    case 'command': return 'Command approval';
    case 'file_change': return 'File change approval';
    case 'question': return 'Question from the agent';
    case 'tool': return 'Tool approval';
    default: return 'Pending request';
  }
}

function pendingRequest(id: string, value: unknown): PendingRequest {
  const detail = record(value);
  const questions = Array.isArray(detail['questions']) ? detail['questions'] : [];
  return {
    id,
    kind: text(detail['kind']),
    questions: questions.map((value: unknown) => {
      const q = record(value);
      return { key: text(q['key']), text: text(q['text']), choices: Array.isArray(q['choices']) ? q['choices'].filter((v): v is string => typeof v === 'string') : [], many: q['many'] === true };
    }).filter((q) => q.key !== ''),
    title: requestTitle(detail['kind']),
    subject: questions.map((question: unknown) => text(record(question)['text'])).filter(Boolean).join('\n')
      || text(detail['subject']),
    diff: text(detail['diff']),
  };
}

function appendReply(event: Record<string, unknown>, sequence: string, messages: Message[], draft: Message | null): Message | null {
  const reply = text(event['text']);
  switch (event['kind']) {
    case 'text_delta':
      return reply === '' ? draft : { sequence: draft?.sequence ?? sequence, role: 'agent', text: (draft?.text ?? '') + reply };
    case 'message':
      if (reply !== '') messages.push({ sequence: draft?.sequence ?? sequence, role: 'agent', text: reply });
      return null;
    case 'turn_started':
    case 'turn_ended':
      if (draft !== null) messages.push(draft);
      return null;
    default:
      return draft;
  }
}

export function createTranscript() {
  const messages = $state<Message[]>([]);
  const pending = new SvelteMap<string, PendingRequest>();
  let draft = $state.raw<Message | null>(null);

  function append({ sequence, content }: ThreadEntry): void {
    const change = record(content);
    switch (change['change']) {
      case 'omitted':
        if (draft !== null) messages.push(draft);
        draft = null;
        messages.push({ sequence, role: 'notice', text: change['reason'] === 'runner_history_gap' ? 'Some earlier activity is no longer available on this node.' : 'An entry exceeded the sharing limit and was omitted.' });
        break;
      case 'prompt':
        if (draft !== null) messages.push(draft);
        draft = null;
        messages.push({ sequence, role: 'you', text: text(change['text']) || 'Prompt with attachments' });
        break;
      case 'said':
        draft = appendReply(record(change['event']), sequence, messages, draft);
        break;
      case 'asked': {
        const id = text(change['request']);
        if (id !== '') pending.set(id, pendingRequest(id, change['detail']));
        break;
      }
      case 'open_requests': {
        const open = new Set(Array.isArray(change['requests']) ? change['requests'].filter((id): id is string => typeof id === 'string') : []);
        for (const id of pending.keys()) if (!open.has(id)) pending.delete(id);
        break;
      }
      case 'closed':
        pending.delete(text(change['request']));
        break;
    }
  }

  return {
    get messages(): readonly Message[] { return draft === null ? messages : [...messages, draft]; },
    get pending(): readonly PendingRequest[] { return [...pending.values()]; },
    append,
  };
}

export function updatedState(entry: ThreadEntry): string | undefined {
  const change = record(entry.content);
  return change['change'] === 'state' ? text(change['state']) || undefined : undefined;
}

export function stateLabel(state: string): string {
  switch (state) {
    case 'needs_you': return 'Needs you';
    case 'starting': return 'Starting';
    case 'working': return 'Working';
    case 'idle': return 'Idle';
    case 'stopped': return 'Stopped';
    default: return 'Unknown';
  }
}

export function threadTitle(title: string): string {
  return title.trim() || 'Untitled thread';
}
