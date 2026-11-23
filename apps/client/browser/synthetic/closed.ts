import type { BrowserContext } from '@playwright/test';

// playwright-core 1.65 does not name every closed-target error `TargetClosedError`; a protocol error carries only text.

const CLOSED_TEXT = ['has been closed', 'Could not find browser context', 'Failed to find browser context'];

export function isClosedError(error: unknown): boolean {
  if (!(error instanceof Error)) return false;
  return error.name === 'TargetClosedError' || CLOSED_TEXT.some((text) => error.message.includes(text));
}

const closedContexts = new WeakSet<BrowserContext>();

export function noteClosing(context: BrowserContext): void {
  context.on('close', () => closedContexts.add(context));
}

export async function untilClosed(answer: () => Promise<void>, context?: BrowserContext): Promise<void> {
  try {
    await answer();
  } catch (error) {
    const contextGone = context !== undefined && closedContexts.has(context);
    if (!contextGone && !isClosedError(error)) throw error;
  }
}
