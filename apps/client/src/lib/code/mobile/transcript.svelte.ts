import { SvelteMap } from 'svelte/reactivity';
import type { ThreadEntry, ThreadSummary } from './api';

export interface Message {
  readonly sequence: string;
  readonly role: 'you' | 'agent';
  readonly text: string;
}

interface PendingRequest {
  readonly id: string;
  readonly sequence: number;
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

function pendingRequest(id: string, sequence: number, value: unknown): PendingRequest {
  const detail = record(value);
  const questions = Array.isArray(detail['questions']) ? detail['questions'] : [];
  return {
    id,
    sequence,
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

function attentionCleared(state: string): boolean {
  return state === 'idle' || state === 'starting' || state === 'working' || state === 'stopped';
}

export function createConversation() {
  const messages = $state<Message[]>([]);
  const pending = new SvelteMap<string, PendingRequest>();
  let draft = $state.raw<Message | null>(null);
  let published = $state.raw<{ summary: ThreadSummary; through: number } | null>(null);
  let streamed = $state.raw<{ state: string; sequence: number } | null>(null);
  let requestsClearedThrough = -1;

  function acceptSummary(summary: ThreadSummary, lastSequence: string): void {
    const through = Number(lastSequence);
    if (published !== null && through < published.through) return;
    // The summary covers status through this sequence; replay still supplies transcript entries.
    published = { summary, through };
    if (streamed !== null && streamed.sequence <= through) streamed = null;
    if (!attentionCleared(summary.state)) return;
    requestsClearedThrough = through;
    for (const request of pending.values()) {
      if (request.sequence <= through) pending.delete(request.id);
    }
  }

  function append({ sequence, content }: ThreadEntry): void {
    const change = record(content);
    switch (change['change']) {
      case 'prompt':
        if (draft !== null) messages.push(draft);
        draft = null;
        messages.push({ sequence, role: 'you', text: text(change['text']) || 'Prompt with attachments' });
        break;
      case 'said':
        draft = appendReply(record(change['event']), sequence, messages, draft);
        break;
      case 'state': {
        const state = text(change['state']);
        const position = Number(sequence);
        if (state !== '' && position > (published?.through ?? -1)) streamed = { state, sequence: position };
        break;
      }
      case 'asked': {
        const id = text(change['request']);
        const position = Number(sequence);
        if (id !== '' && position > requestsClearedThrough) pending.set(id, pendingRequest(id, position, change['detail']));
        break;
      }
      case 'closed':
        pending.delete(text(change['request']));
        break;
    }
  }

  return {
    get summary() { return published?.summary ?? null; },
    get state() { return streamed?.state ?? published?.summary.state ?? ''; },
    get messages(): readonly Message[] { return draft === null ? messages : [...messages, draft]; },
    get pending(): readonly PendingRequest[] { return [...pending.values()]; },
    acceptSummary,
    append,
  };
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
