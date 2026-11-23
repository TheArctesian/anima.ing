export interface Violation {
  path: string;
  signal: string;
  detail: string;
}

export function formatViolation(violation: Violation): string {
  return `${violation.path}: ${violation.signal} ${violation.detail}`;
}

export function report(check: string, violations: Violation[]): number {
  for (const violation of violations) {
    console.error(formatViolation(violation));
  }
  if (violations.length > 0) {
    console.error(`${check}: ${violations.length} violation(s)`);
    return 1;
  }
  console.log(`${check}: ok`);
  return 0;
}
