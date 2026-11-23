export function dayCount(days: number): string {
  return `${String(days)} ${Math.abs(days) === 1 ? 'day' : 'days'}`;
}
