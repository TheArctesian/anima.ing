/** Calls during a run become one more run after it, unless `stopped()` says the owner is gone. */
export function coalesced(run: () => Promise<void>, stopped: () => boolean): () => Promise<void> {
  let running = false;
  let again = false;
  return async () => {
    if (running) {
      again = true;
      return;
    }
    running = true;
    try {
      do {
        again = false;
        await run();
      } while (again && !stopped());
    } finally {
      running = false;
      again = false;
    }
  };
}
