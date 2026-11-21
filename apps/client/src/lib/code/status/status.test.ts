import { describe, expect, it } from 'vitest';
import { STATUS_LOOKS, shortAge, type ChatStatus } from './status';

describe('the status of a chat (D11)', () => {
  it('maps each state to its word and status token', () => {
    const tones = Object.fromEntries(Object.entries(STATUS_LOOKS).map(([status, look]) => [status, `${look.label}/${look.tone}`]));
    expect(tones).toEqual({
      preparing: 'Setting up/accent',
      working: 'Working/accent',
      waiting: 'Waiting/secondary',
      approval: 'Approval/degraded',
      input: 'Input/degraded',
      request: 'Request/degraded',
      limited: 'Limited/degraded',
      failed: 'Failed/down',
      woke: 'Woke/degraded',
      done: 'Done/up',
    } satisfies Record<ChatStatus, string>);
  });

  it('writes the age of a chat with no status in its short form', () => {
    const now = Date.UTC(2026, 10, 21, 12);
    expect(shortAge(now - 59_000, now)).toBe('now');
    expect(shortAge(now - 5 * 60_000, now)).toBe('5m');
    expect(shortAge(now - 3 * 3_600_000 - 1, now)).toBe('3h');
    expect(shortAge(now - 2 * 86_400_000, now)).toBe('2d');
    expect(shortAge(now + 1_000, now)).toBe('now');
  });
});
