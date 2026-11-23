import { describe, expect, it } from 'vitest';
import { noticeWords } from './update-notice';

describe('noticeWords (HMD-055)', () => {
  it('HMD-055: an update names both versions', () => {
    expect(noticeWords([{ kind: 'updated', from: '0.9.29', to: '0.10.72' }])).toContain('updated from 0.9.29 to 0.10.72');
  });

  it('HMD-055: a failed update says its step and that the daemon of before runs, and the older daemon is not said twice', () => {
    const words = noticeWords([
      { kind: 'not_updated', from: '0.9.29', to: '0.10.72', step: 'unhealthy' },
      { kind: 'older', installed: '0.9.29', app: '0.10.72', why: 'unhealthy' },
    ]);
    expect(words).toContain('was not updated to 0.10.72: The new daemon did not answer after its start');
    expect(words).toContain('(0.9.29) runs again');
    expect(words).not.toContain('is older than this app');
  });

  it('HMD-055: an older daemon says the way to update it', () => {
    expect(noticeWords([{ kind: 'older', installed: '0.9.29', app: '0.10.72', why: 'copy' }])).toContain('Quit Anima and open it again');
    expect(noticeWords([{ kind: 'older', installed: null, app: '0.10.72', why: 'not_sealed' }])).toContain('Install the app again');
  });

  it('HMD-055: no notice, no words', () => {
    expect(noticeWords([])).toBeNull();
  });
});
