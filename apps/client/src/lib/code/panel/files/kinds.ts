import type { CodeIconName } from '../../icons/icons';

/**
 * What a file of the right panel is, and which viewer shows it (t3-ui-copy-2026-11-21/05, Sections 3.4, 3.6, 3.7 and
 * 3.9). No drawing here: only the lists, the order of the decision and the words.
 */
export type FileKind =
  'markdown' | 'mermaid' | 'table' | 'html' | 'image' | 'pdf' | 'archive' | 'sqlite' | 'office' | 'notebook' | 'font' | 'audio' | 'video' | 'text';

/** The extensions of each kind (3.4 and 3.6; the image and audio lists are those of T3 Code's `mediaPreview.ts`). */
const EXTENSIONS: Readonly<Record<Exclude<FileKind, 'text'>, readonly string[]>> = {
  markdown: ['md', 'mdx', 'markdown', 'mdown', 'mkd'],
  mermaid: ['mmd', 'mermaid'],
  table: ['csv', 'tsv'],
  html: ['htm', 'html'],
  image: ['avif', 'bmp', 'gif', 'ico', 'heic', 'heif', 'jpeg', 'jpg', 'png', 'svg', 'tif', 'tiff', 'webp'],
  pdf: ['pdf'],
  archive: ['zip', 'jar', 'tar', 'gz', 'tgz'],
  sqlite: ['sqlite', 'sqlite3', 'db'],
  office: ['docx', 'xlsx', 'pptx', 'odt', 'ods', 'odp', 'rtf', 'epub'],
  notebook: ['ipynb'],
  font: ['ttf', 'otf', 'woff', 'woff2'],
  audio: ['mp3', 'wav', 'ogg', 'oga', 'flac', 'aac', 'm4a', 'opus', 'aiff', 'weba'],
  video: ['mp4', 'm4v', 'mov', 'webm', 'mkv'],
};

const KIND_BY_EXTENSION = new Map<string, FileKind>(
  Object.entries(EXTENSIONS).flatMap(([kind, list]) => list.map((extension) => [extension, kind as FileKind])),
);

/** The name of each kind, for the block that says a kind cannot be shown yet. */
export const KIND_NAMES: Readonly<Record<FileKind, string>> = {
  markdown: 'Markdown',
  mermaid: 'Mermaid diagram',
  table: 'Table',
  html: 'HTML page',
  image: 'Image',
  pdf: 'PDF',
  archive: 'Archive',
  sqlite: 'SQLite database',
  office: 'Office document',
  notebook: 'Notebook',
  font: 'Font',
  audio: 'Audio',
  video: 'Video',
  text: 'Text',
};

/** The last part of a path: the name of the file. */
export function baseName(path: string): string {
  return path.slice(path.lastIndexOf('/') + 1);
}

/** The kind of a file from the extension of its path; a path with no known extension is text. */
export function fileKind(path: string): FileKind {
  const name = baseName(path);
  const dot = name.lastIndexOf('.');
  if (dot < 0) return 'text';
  return KIND_BY_EXTENSION.get(name.slice(dot + 1).toLowerCase()) ?? 'text';
}

/** The text limit of a file read: 1 MB (T3 Code's `FILE_TEXT_PREVIEW_MAX_BYTES`). */
export const FILE_TEXT_PREVIEW_MAX_BYTES = 1_048_576;

/** The notice over a file that was cut at the limit. */
export function limitNotice(bytes: number): string {
  return `Preview limited to the first 1 MB of a ${bytes.toLocaleString('en-US')} byte file.`;
}

/** The notice over a file while the connection cannot write. */
export const READ_ONLY_NOTICE = 'Read-only connection. Unsaved edits are kept until write access returns.';

/** Why a file could not be read (the failures of T3 Code's `ProjectReadFileError`). */
export type ReadFailure =
  'path_not_file' | 'binary_file' | 'workspace_path_outside_root' | 'resolved_path_outside_root' | 'workspace_root' | 'operation_failed';

/** The words of each read failure (3.9). */
export const READ_ERRORS: Readonly<Record<ReadFailure, string>> = {
  path_not_file: 'The path is a directory or special file, not a regular file.',
  binary_file: 'The file is binary and cannot be displayed as text.',
  workspace_path_outside_root: 'The requested path is outside the workspace.',
  resolved_path_outside_root: 'The path resolves to a location outside the workspace.',
  workspace_root: 'The workspace folder could not be accessed.',
  operation_failed: 'The file could not be accessed or read. It may be missing or inaccessible.',
};

/** The label of a line range of a comment or a link: "L12", or "L12 to L20". */
export function rangeLabel(start: number, end: number = start): string {
  const [from, to] = start <= end ? [start, end] : [end, start];
  return from === to ? `L${String(from)}` : `L${String(from)} to L${String(to)}`;
}

/** The rendered / source toggle of one kind (3.4): its two labels and what shows first. */
export interface RenderToggle {
  /** The label while the rendered view shows; a press shows the source. */
  readonly toSource: string;
  /** The label while the source shows; a press shows the rendered view. */
  readonly toRendered: string;
  readonly renderedByDefault: boolean;
}

const TOGGLES: Readonly<Partial<Record<FileKind, RenderToggle>>> = {
  markdown: { toSource: 'Show markdown source', toRendered: 'Show rendered markdown', renderedByDefault: false },
  mermaid: { toSource: 'Show Mermaid source', toRendered: 'Show rendered diagram', renderedByDefault: true },
  table: { toSource: 'Show source', toRendered: 'Show table', renderedByDefault: true },
  html: { toSource: 'Show HTML source', toRendered: 'Show rendered page', renderedByDefault: true },
};

/** The toggle of a kind, or `undefined` for a kind with one view only. */
export function renderToggle(kind: FileKind): RenderToggle | undefined {
  return TOGGLES[kind];
}

/**
 * The kinds whose rendered view this client can draw now. Mermaid, tables and HTML pages wait for their viewers, so
 * they show their source and have no toggle yet.
 */
export const RENDERABLE: readonly FileKind[] = ['markdown'];

/** What the person chose for a file, and how it was opened. */
export interface RenderChoice {
  /** The kept choice for this kind, or `undefined` when the person never chose. */
  readonly kept?: boolean | undefined;
  /** The tab was opened at a line number. */
  readonly atLine?: boolean;
  /** The person pressed the toggle in this tab. */
  readonly pressed?: boolean;
}

/** The rendered view shows: the kept choice or the default, but a file opened at a line shows its source until the toggle is pressed. */
export function showsRendered(kind: FileKind, choice: RenderChoice = {}): boolean {
  const toggle = renderToggle(kind);
  if (toggle === undefined || !RENDERABLE.includes(kind)) return false;
  if (choice.atLine === true && choice.pressed !== true) return false;
  return choice.kept ?? toggle.renderedByDefault;
}

/** The icon and the label of the toggle for what shows now. */
export function toggleFace(kind: FileKind, rendered: boolean): { readonly label: string; readonly icon: CodeIconName } | undefined {
  const toggle = renderToggle(kind);
  if (toggle === undefined) return undefined;
  if (rendered) return { label: toggle.toSource, icon: 'code-xml' };
  return { label: toggle.toRendered, icon: kind === 'table' ? 'table-2' : 'eye' };
}

/** The viewers this client has. `unsupported` is the plain block that names the kind. */
export type Viewer = 'image' | 'markdown' | 'source' | 'error' | 'loading' | 'unsupported';

/** What is known of a file when its viewer is chosen. */
export interface ViewerInput {
  readonly path: string;
  readonly attachment?: boolean;
  /** The server says the content is binary. */
  readonly binary?: boolean;
  readonly failed?: boolean;
  readonly loading?: boolean;
  /** The rendered view is asked for (see `showsRendered`). */
  readonly rendered?: boolean;
}

const NEVER_TEXT: readonly FileKind[] = ['video', 'audio', 'pdf', 'archive', 'sqlite', 'office', 'notebook', 'font'];

/**
 * The viewer of a file, in the order of T3 Code (3.6): attachment, media, image, documents and binary kinds, then the
 * read error, the loading state, the rendered views and the source. A kind with no viewer here gives `unsupported`.
 */
export function viewerFor(input: ViewerInput): Viewer {
  const kind = fileKind(input.path);
  if (kind === 'image') return 'image';
  if (input.attachment === true || NEVER_TEXT.includes(kind) || input.binary === true) return 'unsupported';
  if (input.failed === true) return 'error';
  if (input.loading === true) return 'loading';
  if (kind === 'markdown' && input.rendered === true) return 'markdown';
  return 'source';
}

/** What the toolbar of a file shows (3.4), from what is known of the file. */
export interface ToolbarInput extends ViewerInput {
  readonly text?: string | undefined;
  readonly truncated?: boolean;
  /** The file is outside the workspace (a host path). */
  readonly host?: boolean;
  /** The host can open a file in the preview browser. */
  readonly canPreview?: boolean;
}

/** Which controls the toolbar shows: the toggle, wrap, copy, the preview browser and the explorer toggle. */
export function toolbarShows(input: ToolbarInput): { toggle: boolean; wrap: boolean; copy: boolean; preview: boolean; explorer: boolean } {
  const kind = fileKind(input.path);
  const viewer = viewerFor(input);
  const text = input.text ?? '';
  return {
    toggle: RENDERABLE.includes(kind) && (viewer === 'markdown' || viewer === 'source'),
    wrap: viewer === 'source',
    copy: (viewer === 'source' || viewer === 'markdown') && input.truncated !== true && text !== '',
    preview: input.canPreview === true && (kind === 'html' || kind === 'pdf'),
    explorer: input.attachment !== true && input.host !== true,
  };
}

/** The words of the plain block for a file with no viewer here yet. */
export function unsupportedWords(input: ViewerInput): string {
  const kind = fileKind(input.path);
  if (kind === 'text' || !NEVER_TEXT.includes(kind)) {
    return input.attachment === true ? 'This attachment cannot be shown here yet.' : 'This file is binary and cannot be shown here yet.';
  }
  return `${KIND_NAMES[kind]} files cannot be shown here yet.`;
}

/** A read of a file, as the host gives it to the File surface. */
export type FileRead =
  | { readonly state: 'loading' }
  | { readonly state: 'failed'; readonly failure: ReadFailure; /** The words of the server for a failure with no words here. */ readonly message?: string }
  | {
      readonly state: 'ready';
      /** The text of a text file (the first 1 MB when `truncated`). */
      readonly text?: string;
      /** Where an image is read from. */
      readonly src?: string;
      readonly binary?: boolean;
      /** The size of the whole file. */
      readonly bytes?: number;
      readonly truncated?: boolean;
    };
