// Bun.spawnSync with piped output can busy-wait on a child that has exited (Bun 1.3.11, R-112).
export interface FinishedProcess {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export interface FinishOptions {
  env?: Record<string, string | undefined>;
  stdin?: string;
}

/** Reads stdout and stderr while it waits for the exit, so a full pipe cannot stall the child. */
export async function finish(command: string[], options: FinishOptions = {}): Promise<FinishedProcess> {
  const child = Bun.spawn(command, {
    env: options.env,
    stdin: options.stdin === undefined ? "ignore" : Buffer.from(options.stdin),
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr] = await Promise.all([new Response(child.stdout).text(), new Response(child.stderr).text(), child.exited]);
  return { exitCode: child.exitCode ?? 1, stdout, stderr };
}
