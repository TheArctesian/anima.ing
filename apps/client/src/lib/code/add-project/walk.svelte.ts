/** Where the "Add project" palette is: its list of sources, or a folder of "Local folder" with the folders in it. */
import type { CallResult } from '$lib/agent-threads/live/live.svelte';
import type { PaletteGroup, PaletteRow } from '../palette/results';
import { inside, parentOf, shownFolders, shownSources } from './sources';

/** The folders in a folder as the runner gives them (`folders`, TH113), or the words of why it did not. */
export type FolderList = { readonly ok: true; readonly folder: string; readonly folders: readonly string[]; readonly more: boolean } | { readonly ok: false; readonly words: string };

/** The runner's answer to `folders` as a list, or why there is none. */
export function folderListOf(result: CallResult): FolderList {
  if (!result.ok) return { ok: false, words: result.words };
  const { outcome } = result;
  return outcome.outcome === 'folders' ? { ok: true, folder: outcome.folder, folders: outcome.folders, more: outcome.more } : { ok: false, words: 'The thread runner did not list the folders.' };
}

export class AddProjectWalk {
  /** null: the list of sources; a path: the folder that "Local folder" shows. */
  folder = $state<string | null>(null);
  query = $state('');
  folders = $state<readonly string[]>([]);
  more = $state(false);
  loading = $state(false);
  problem = $state<string | null>(null);
  #asked = 0;
  readonly #list: (folder: string) => Promise<FolderList>;
  readonly #start: () => string;
  readonly #add: (folder: string) => void;

  /** `start`: where "Local folder" opens; `add`: takes the chosen folder. */
  constructor(list: (folder: string) => Promise<FolderList>, start: () => string, add: (folder: string) => void, private readonly create?: () => void) {
    this.#list = list;
    this.#start = start;
    this.#add = add;
  }

  /** Shows the folders of `folder`; a later step wins over an answer that comes late. */
  async browse(folder: string): Promise<void> {
    const asked = ++this.#asked;
    this.folder = folder;
    this.query = '';
    this.loading = true;
    this.problem = null;
    const listed = await this.#list(folder);
    if (asked !== this.#asked) return;
    this.loading = false;
    if (!listed.ok) {
      this.folders = [];
      this.problem = listed.words;
      return;
    }
    this.folder = listed.folder;
    this.folders = listed.folders;
    this.more = listed.more;
  }

  /** Backspace in an empty search, or the back arrow: up one folder, then back to the sources. */
  back(): void {
    if (this.folder === null) return;
    if (this.folder === '/' || this.problem !== null) {
      this.#asked += 1;
      this.folder = null;
      this.query = '';
      this.problem = null;
      this.loading = false;
      return;
    }
    void this.browse(parentOf(this.folder));
  }

  /** Each opening starts at the sources. */
  reset(): void {
    this.#asked += 1;
    this.folder = null;
    this.query = '';
    this.problem = null;
    this.loading = false;
  }

  get groups(): readonly PaletteGroup[] {
    return this.folder === null ? [this.#sources()] : [this.#walk(this.folder)];
  }

  #sources(): PaletteGroup {
    const rows = shownSources(this.query).map<PaletteRow>((source) => ({
      id: `source:${source.id}`,
      title: source.title,
      icon: source.icon,
      detail: source.needs === null ? source.line : `${source.line}. ${source.needs}`,
      disabled: source.needs !== null || (source.id === 'new' && this.create === undefined),
      run: () => source.id === 'new' ? this.create?.() : void this.browse(this.#start()),
    }));
    return { id: 'sources', label: 'Sources', rows };
  }

  #walk(folder: string): PaletteGroup {
    const typed = this.query.trim();
    if (typed.startsWith('/')) {
      return { id: 'folders', label: folder, rows: [{ id: 'go', title: `Go to ${typed}`, icon: 'folder', disabled: false, run: () => void this.browse(typed) }] };
    }
    const here: PaletteRow = { id: 'add', title: 'Add this folder', icon: 'folder-plus', detail: folder, disabled: this.problem !== null, run: () => {
        this.#add(folder);
      }, };
    const below = shownFolders(this.folders, typed).map<PaletteRow>((name) => ({ id: `folder:${name}`, title: name, icon: 'folder', disabled: false, run: () => void this.browse(inside(folder, name)) }));
    return { id: 'folders', label: folder, rows: typed === '' ? [here, ...below] : below };
  }
}
