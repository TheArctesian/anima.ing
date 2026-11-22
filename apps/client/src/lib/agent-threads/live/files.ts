/** A file or a folder of the working folder of a thread, by its path from that folder (TH108). */
export interface FoundFile {
  readonly path: string;
  readonly folder: boolean;
}

/** The paths of a `files` answer of the runner, or `null` when one does not read. */
export function filesFrom(value: unknown): FoundFile[] | null {
  if (!Array.isArray(value)) return null;
  const paths: FoundFile[] = [];
  for (const found of value as unknown[]) {
    if (typeof found !== 'object' || found === null) return null;
    const { path, folder } = found as { readonly path?: unknown; readonly folder?: unknown };
    if (typeof path !== 'string' || typeof folder !== 'boolean') return null;
    paths.push({ path, folder });
  }
  return paths;
}
