import type { MetricsSource } from '$lib/hub/generated/graphql';

/** SSH polling (MET-004) or the daemon's pushes (MET-011). */
export function hasMetrics(source: MetricsSource | null | undefined): boolean {
  return source === 'SSH' || source === 'DAEMON';
}
