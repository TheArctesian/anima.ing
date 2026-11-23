function object(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : {};
}

function text(value: unknown): string | null {
  return typeof value === 'string' ? value.slice(0, 512) : null;
}

export function updateStatus(value: unknown) {
  const state = object(value);
  const remote = object(state['remote']);
  const jobs = object(remote['jobs']);
  const result = object(remote['result']);
  const checked = object(result['value']);
  const operation = object(state['operation']);
  const running = Object.values(jobs).some((job) => job === 'active' || job === 'activating');
  const failed = Object.values(jobs).some((job) => job === 'failed');
  const phase = text(state['phase']) ?? 'unknown';
  const bytes = operation['bytes'];
  const total = operation['totalBytes'];
  const progress = typeof bytes === 'number' && typeof total === 'number' && total > 0 ? Math.min(100, Math.max(0, bytes / total * 100)) : null;
  return {
    phase,
    running,
    progress,
    version: text(state['version']) ?? text(object(checked['release'])['version']),
    available: checked['available'] === true,
    rollback: typeof state['previous'] === 'string',
    message: text(result['error']) ?? (failed ? 'The update request failed. Refresh the status and retry.' : running ? 'Updating. You can leave this page; work continues on the device.' : phase === 'staged' ? 'Update downloaded and verified. Ready to restart.' : phase === 'booting' ? 'Restarting and checking the updated system.' : phase === 'current' ? 'The installed OS update passed its startup checks.' : phase === 'rolled-back' ? 'The previous OS was restored.' : checked['available'] === false ? 'The OS is up to date.' : 'Ready to check for updates.'),
  };
}
