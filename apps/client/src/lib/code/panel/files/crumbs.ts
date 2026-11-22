import type { MenuEntry } from '../../kit/menu';

/**
 * The breadcrumbs of a file and the menu of a folder (t3-ui-copy-2026-11-21/05, Section 3.5). A path here is relative
 * to the workspace folder, with `/` between its parts; the workspace folder itself is the empty path.
 */
export interface Crumb {
  /** The name that shows: the project name for the root, else the name of the part. */
  readonly label: string;
  /** The path up to this part; empty for the root. */
  readonly path: string;
  /** `above` the shown path, the shown path itself (`current`), or `below` it (kept after the person went up). */
  readonly place: 'above' | 'current' | 'below';
  /** A folder has a separator after it that opens its menu; only the last crumb can be a file. */
  readonly folder: boolean;
}

/** The parts of a path, with no empty part and no "." part. */
export function pathParts(path: string): readonly string[] {
  return path.split('/').filter((part) => part !== '' && part !== '.');
}

/** The folder that holds a path; empty for a path at the root. */
export function parentPath(path: string): string {
  return pathParts(path).slice(0, -1).join('/');
}

/** How the crumbs are asked for, beyond the path that shows. */
export interface CrumbOptions {
  /** The deepest path the person came from; its parts stay after the shown path so the person can go back down. */
  readonly trail?: string | undefined;
  /** The deepest path is a folder (the tab shows the tree of a folder), not a file. */
  readonly folder?: boolean;
}

/** The crumbs of `path`, starting with the project name. */
export function crumbsFor(project: string, path: string, options: CrumbOptions = {}): readonly Crumb[] {
  const shown = pathParts(path);
  const trail = pathParts(options.trail ?? '');
  const keepsTrail = trail.length > shown.length && shown.every((part, index) => trail[index] === part);
  const parts = keepsTrail ? trail : shown;
  const place = (depth: number): Crumb['place'] => (depth < shown.length ? 'above' : depth === shown.length ? 'current' : 'below');
  const crumbs: Crumb[] = [{ label: project, path: '', place: place(0), folder: true }];
  parts.forEach((label, index) => {
    const last = index === parts.length - 1;
    crumbs.push({ label, path: parts.slice(0, index + 1).join('/'), place: place(index + 1), folder: !last || options.folder === true });
  });
  return crumbs;
}

/** One entry of a folder, as the host lists it. */
export interface FolderEntry {
  readonly name: string;
  readonly path: string;
  readonly folder: boolean;
}

/** What the host knows of a folder when its menu opens. */
export type FolderListing =
  | { readonly state: 'loading' }
  | { readonly state: 'failed' }
  | { readonly state: 'gone' }
  | {
      readonly state: 'ready';
      readonly entries: readonly FolderEntry[];
      /** A refresh failed and these are the old entries. */
      readonly stale?: boolean;
      /** The index of the workspace is partial: some entries are not listed. */
      readonly partial?: boolean;
    };

/** The IDs of the menu rows that are not an entry. */
export const MENU_BACK = 'back';
export const MENU_RETRY = 'retry';

/** What the menu is asked to show. */
export interface FolderMenuInput {
  /** The folder the separator opened, and the folder that shows now (deeper after the person stepped in). */
  readonly root: string;
  readonly folder: string;
  /** The name of the project, for "Back to" at the workspace folder. */
  readonly project: string;
  /** The path of the file that is open, which the menu marks. */
  readonly open?: string;
}

function note(label: string): MenuEntry {
  return { kind: 'choice', id: `note:${label}`, label, disabled: true };
}

function entryRows(listing: Extract<FolderListing, { state: 'ready' }>, open: string | undefined): MenuEntry[] {
  if (listing.entries.length === 0) {
    return [note(listing.partial === true ? 'No entries from this folder are available in the partial workspace index.' : 'This folder is empty.')];
  }
  const folders = listing.entries.filter((entry) => entry.folder);
  const files = listing.entries.filter((entry) => !entry.folder);
  return [
    ...folders.map((entry): MenuEntry => ({ kind: 'choice', id: `folder:${entry.path}`, label: entry.name, icon: 'folder' })),
    ...files.map((entry): MenuEntry => ({ kind: 'choice', id: `file:${entry.path}`, label: entry.name, icon: entry.path === open ? 'check' : 'file' })),
  ];
}

/** The rows of the menu of a folder (3.5): "Back to", the folders, the files, and the rows of its states. */
export function folderMenu(listing: FolderListing, input: FolderMenuInput): readonly MenuEntry[] {
  const rows: MenuEntry[] = [];
  if (input.folder !== input.root) {
    const parent = pathParts(input.folder).at(-2) ?? input.project;
    rows.push({ kind: 'choice', id: MENU_BACK, label: `Back to ${parent}`, icon: 'arrow-left' }, { kind: 'line' });
  }
  if (listing.state === 'loading') return [...rows, note('Loading folder…')];
  if (listing.state === 'failed') return [...rows, { kind: 'choice', id: MENU_RETRY, label: 'Retry loading folder', icon: 'refresh-cw' }];
  if (listing.state === 'gone') return [...rows, note('This folder is no longer available.')];
  rows.push(...entryRows(listing, input.open));
  if (listing.stale === true) rows.push({ kind: 'line' }, { kind: 'choice', id: MENU_RETRY, label: 'Refresh failed. Retry', icon: 'refresh-cw' });
  if (listing.partial === true && listing.entries.length > 0) rows.push({ kind: 'line' }, note('Some workspace entries are not shown.'));
  return rows;
}

/** What a chosen row of the folder menu asks for. */
export type FolderMenuIntent =
  | { readonly kind: 'back' }
  | { readonly kind: 'retry' }
  | { readonly kind: 'enter'; readonly path: string }
  | { readonly kind: 'open'; readonly path: string };

/** The intent of the row with this ID; `null` for a row that does nothing. */
export function folderMenuIntent(id: string): FolderMenuIntent | null {
  if (id === MENU_BACK) return { kind: 'back' };
  if (id === MENU_RETRY) return { kind: 'retry' };
  if (id.startsWith('folder:')) return { kind: 'enter', path: id.slice('folder:'.length) };
  if (id.startsWith('file:')) return { kind: 'open', path: id.slice('file:'.length) };
  return null;
}
