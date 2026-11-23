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

/** What the runner says of a folder: its notice (THR-046), the files that match (TH108), or the folders in it (TH113). */
export type FolderOutcome =
  | { readonly outcome: 'folder'; readonly notice: readonly string[] }
  /** The best match first; `more` when more matched. */
  | { readonly outcome: 'files'; readonly paths: readonly FoundFile[]; readonly more: boolean }
  /** `folder` is its real path; the names are sorted; `more` when it held more than the answer gives. */
  | { readonly outcome: 'folders'; readonly folder: string; readonly folders: readonly string[]; readonly more: boolean };

function names(value: unknown): readonly string[] | null {
  return Array.isArray(value) && value.every((name) => typeof name === 'string') ? (value as string[]) : null;
}

/** A `folder`, `files` or `folders` answer of the runner, or `null` when it does not read. */
export function folderOutcomeFrom(value: Readonly<Record<string, unknown>>): FolderOutcome | null {
  if (value.outcome === 'folders') {
    const folders = names(value.folders);
    return folders === null || typeof value.folder !== 'string' ? null : { outcome: 'folders', folder: value.folder, folders, more: value.more === true };
  }
  if (value.outcome === 'files') {
    const paths = filesFrom(value.paths);
    return paths === null ? null : { outcome: 'files', paths, more: value.more === true };
  }
  const notice = names(value.notice);
  return notice === null ? null : { outcome: 'folder', notice };
}
