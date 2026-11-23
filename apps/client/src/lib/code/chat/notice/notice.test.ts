import { describe, expect, it } from 'vitest';
import { noticeOf, stopWords } from './notice';

const OLDER =
  'The holder of the threads is older than the daemon, so the corrections of this build are not in force for its programs. It is replaced by itself when no thread runs (2 running now); stop them to replace it at once.';

describe('noticeOf', () => {
  it('makes the older holder one short line, keeps the full words for the tip, and counts the running threads', () => {
    const notice = noticeOf(OLDER);
    expect(notice.line.length).toBeLessThan(90);
    expect(notice.full).toBe(OLDER);
    expect(notice.running).toBe(2);
    expect(stopWords(notice.running)).toBe('Stop the 2 running threads and replace');
  });

  it('shows other words as they are, with no stop', () => {
    expect(noticeOf('The thread runner of this computer is not running.')).toEqual({ line: 'The thread runner of this computer is not running.', full: null, running: 0 });
  });
});
