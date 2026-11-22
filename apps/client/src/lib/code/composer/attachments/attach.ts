import type { CodeIconName } from '../../icons/icons';

/** How the composer shows an attachment: a tile for an image or a video, a row of the file list for the rest. */
export type AttachmentKind = 'image' | 'video' | 'text' | 'file';

/** Whether the file can be sent: read, still being read, or failed with the words to show. */
export type AttachmentState = { readonly state: 'ready' } | { readonly state: 'reading' } | { readonly state: 'failed'; readonly words: string };

/** One file attached to the draft (03, "Attachments"). */
export interface Attachment {
  readonly id: string;
  readonly name: string;
  readonly kind: AttachmentKind;
  /** In bytes. */
  readonly size: number;
  readonly mime: string;
  /** An object or data URL the tile and the preview dialog draw; null when there is nothing to draw. */
  readonly preview: string | null;
  readonly state: AttachmentState;
}

/** What `classify` and `admit` read of a `File`; a `File` fits as it is. */
export interface FileFacts {
  readonly name: string;
  readonly type: string;
  readonly size: number;
}

/** The kind and the MIME type a file is attached with, or `unsupported` for an image nothing can show or send. */
export type Classified = { readonly kind: AttachmentKind; readonly mime: string } | { readonly kind: 'unsupported' };

/** 03 "Limits". `fileBytes` is null while the server takes no files that are not images. */
export interface Limits {
  readonly count: number;
  readonly imageBytes: number;
  readonly fileBytes: number | null;
  readonly promptChars: number;
  readonly foldBytes: number;
}

const MB = 1024 * 1024;

export const LIMITS: Limits = { count: 100, imageBytes: 10 * MB, fileBytes: 50 * MB, promptChars: 120_000, foldBytes: 32 * 1024 };

const SENT_AS_IMAGE: Readonly<Record<string, string>> = { gif: 'image/gif', jpeg: 'image/jpeg', jpg: 'image/jpeg', png: 'image/png', webp: 'image/webp' };
const VIDEO: Readonly<Record<string, string>> = {
  avi: 'video/x-msvideo',
  m4v: 'video/mp4',
  mkv: 'video/x-matroska',
  mov: 'video/quicktime',
  mp4: 'video/mp4',
  ogv: 'video/ogg',
  webm: 'video/webm',
};
/* Images that are shown as a file but never sent as an image; SVG, HEIC and the sent types are decided before. */
const SHOWN_AS_FILE = ['avif', 'bmp', 'ico', 'tif', 'tiff'];
/* The systems label a TypeScript `.ts` file as an MPEG transport stream: such a type says nothing. */
const SAYS_NOTHING = new Set(['application/octet-stream', 'binary/octet-stream', 'application/unknown', 'video/mp2t', 'video/vnd.dlna.mpeg-tts']);
const CODE = new Set(['c', 'cpp', 'cs', 'css', 'go', 'h', 'html', 'java', 'js', 'json', 'jsx', 'kt', 'py', 'rb', 'rs', 'sh', 'sql', 'svelte', 'swift', 'toml', 'ts', 'tsx', 'xml', 'yaml', 'yml']);

/** The extension in lower case; none for a name without a dot or one that only starts with it. */
function extension(name: string): string {
  const dot = name.lastIndexOf('.');
  return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase();
}

function bareMime(type: string): string {
  return (type.split(';', 1)[0] ?? '').trim().toLowerCase();
}

function isSvg(file: FileFacts): boolean {
  const mime = bareMime(file.type);
  return mime === 'image/svg+xml' || ((mime === '' || SAYS_NOTHING.has(mime)) && extension(file.name) === 'svg');
}

function isDng(file: FileFacts): boolean {
  return file.name.toLowerCase().endsWith('.dng') || ['image/dng', 'image/x-dng', 'image/x-adobe-dng'].includes(bareMime(file.type));
}

/* The file manager gives no type for a photo or a dragged image; only then does the extension decide. */
function untyped(file: FileFacts): boolean {
  const type = file.type.toLowerCase();
  return type === '' || type === 'application/octet-stream';
}

function isHeic(file: FileFacts): boolean {
  return /^image\/hei[cf]$/i.test(file.type) || (untyped(file) && /\.hei[cf]$/i.test(file.name));
}

function videoMime(file: FileFacts): string | null {
  const mime = bareMime(file.type);
  if (mime.startsWith('video/') && !SAYS_NOTHING.has(mime)) return mime;
  if (mime !== '' && !SAYS_NOTHING.has(mime)) return null;
  return VIDEO[extension(file.name)] ?? null;
}

/** A file that is not sent as an image: a video keeps a video type, a text file is `text`, the rest is `file`. */
function plainFile(file: FileFacts, mime?: string): Classified {
  if (mime !== undefined) return { kind: 'file', mime };
  const video = videoMime(file);
  if (video !== null) return { kind: 'video', mime: video };
  const type = file.type === '' ? 'application/octet-stream' : file.type;
  return { kind: bareMime(type).startsWith('text/') ? 'text' : 'file', mime: type };
}

/**
 * 03 "Classification of a file", in the order of T3's `classifyComposerAttachmentFile` and
 * `composerFileAttachmentMimeType`. `video` and `text` are ours: T3 has `image` and `file` and tells a video by its
 * type where it draws it.
 */
export function classify(file: FileFacts): Classified {
  if (isDng(file)) return plainFile(file, 'image/x-adobe-dng');
  if (isSvg(file)) return plainFile(file, 'image/svg+xml');
  if (isHeic(file)) return { kind: 'image', mime: file.type };
  const guessed = untyped(file) ? SENT_AS_IMAGE[extension(file.name)] : undefined;
  if (guessed !== undefined) return { kind: 'image', mime: guessed };
  if (!file.type.toLowerCase().startsWith('image/')) return plainFile(file);
  if (Object.values(SENT_AS_IMAGE).includes(file.type.toLowerCase())) return { kind: 'image', mime: file.type };
  return SHOWN_AS_FILE.includes(extension(file.name)) ? plainFile(file) : { kind: 'unsupported' };
}

/** 03 "Sizes are shown as": whole KB rounded up, at least 1; MB with one decimal from 1024 KB. */
export function sizeWords(bytes: number): string {
  const kilobytes = Math.max(1, Math.ceil(bytes / 1024));
  return kilobytes >= 1024 ? `${(bytes / MB).toFixed(1)} MB` : `${String(kilobytes)} KB`;
}

function limitWords(bytes: number): string {
  if (bytes >= MB && bytes % MB === 0) return `${String(bytes / MB)} MB`;
  if (bytes >= 1024 && bytes % 1024 === 0) return `${String(bytes / 1024)} KB`;
  return bytes === 1 ? '1 byte' : `${String(bytes)} bytes`;
}

/** 03 "Error and toast strings": the words of each refusal, as T3 writes them. */
export const REFUSAL = {
  full: (max: number) => `You can attach up to ${String(max)} files per message.`,
  unsupported: (name: string) => `'${name}' is not a supported image type. Attach GIF, HEIC, HEIF, JPEG, PNG, or WebP images.`,
  noFiles: () => 'This server does not support file attachments.',
  empty: (name: string) => `'${name}' is empty or could not be read.`,
  tooLarge: (name: string, limit: number) => `'${name}' exceeds the ${limitWords(limit)} attachment limit.`,
  notAnImage: (name: string) => `'${name}' could not be read as an image.`,
  imageTooLarge: (name: string) => `'${name}' is too large to attach.`,
} as const;

export interface Admitted<F extends FileFacts> {
  readonly file: F;
  readonly kind: AttachmentKind;
  readonly mime: string;
}

export interface Refused {
  readonly name: string;
  readonly words: string;
}

/** Why a file of this class cannot be attached, or null. T3 checks the size of a file here and of an image after it compresses it; we do not compress, so both are checked here. */
function refusal(file: FileFacts, kind: AttachmentKind, limits: Limits): string | null {
  if (kind === 'image') return file.size > limits.imageBytes ? REFUSAL.imageTooLarge(file.name) : null;
  if (limits.fileBytes === null) return REFUSAL.noFiles();
  if (file.size <= 0) return REFUSAL.empty(file.name);
  return file.size > limits.fileBytes ? REFUSAL.tooLarge(file.name, limits.fileBytes) : null;
}

/**
 * 03 "Limits": which of the incoming files the draft takes. Every attachment of the draft holds a slot, also one that
 * is still being read. T3 shows only the last refusal; all of them are returned, in order.
 */
export function admit<F extends FileFacts>(current: readonly Attachment[], incoming: readonly F[], limits: Limits = LIMITS): { taken: Admitted<F>[]; refused: Refused[] } {
  const taken: Admitted<F>[] = [];
  const refused: Refused[] = [];
  for (const file of incoming) {
    const sort = classify(file);
    let words: string | null;
    if (current.length + taken.length >= limits.count) words = REFUSAL.full(limits.count);
    else if (sort.kind === 'unsupported') words = REFUSAL.unsupported(file.name);
    else words = refusal(file, sort.kind, limits);
    if (words !== null) refused.push({ name: file.name, words });
    else if (sort.kind !== 'unsupported') taken.push({ file, kind: sort.kind, mime: sort.mime });
  }
  return { taken, refused };
}

/** The attachment of a file the draft took. T3 names a file without a name `image` or `file`. */
export function attachmentOf(taken: Admitted<FileFacts>, id: string, preview: string | null, state: AttachmentState = { state: 'ready' }): Attachment {
  const name = taken.file.name === '' ? (taken.kind === 'image' ? 'image' : 'file') : taken.file.name;
  return { id, name, kind: taken.kind, size: taken.file.size, mime: taken.mime, preview, state };
}

/** The icon of a row of the file list. T3's set of file-type icons is not copied (the icons of the Code section are Lucide's). */
export function fileIcon(attachment: Pick<Attachment, 'kind' | 'name'>): CodeIconName {
  if (attachment.kind === 'image') return 'image';
  if (attachment.kind === 'video') return 'video';
  if (CODE.has(extension(attachment.name))) return 'file-code';
  return attachment.kind === 'text' ? 'file-text' : 'file';
}

/** What a paste holds, read from the clipboard event by the composer. */
export interface ClipboardFacts {
  readonly files: readonly FileFacts[];
  /** How many of the clipboard's entries are folders. */
  readonly folders: number;
  readonly plainText: string;
  /** The paste was made with "paste as text" (Cmd+Shift+V, Ctrl+Shift+V). */
  readonly asText: boolean;
  /** The clipboard holds chips copied from a composer: the editor pastes those itself. */
  readonly chips: boolean;
}

/** What a paste meets in the draft. */
export interface DraftFacts {
  readonly promptLength: number;
  readonly selection: { readonly start: number; readonly end: number };
  /** The names of the attached files, so a folded paste gets a free name. */
  readonly names: readonly string[];
  /** Whether the draft can take one more attachment now: a free slot, and nothing that forbids attaching. */
  readonly room: boolean;
}

export type PasteDecision =
  | { readonly act: 'attach'; readonly folders: boolean }
  | { readonly act: 'fold'; readonly name: string; readonly mime: string }
  | { readonly act: 'refuse'; readonly title: string; readonly description: string }
  | { readonly act: 'editor' };

export const FOLDED_MIME = 'text/plain;charset=utf-8';
const TO_EDITOR: PasteDecision = { act: 'editor' };

/** 03 "Pasted text files are named": `pasted-text.txt`, then `pasted-text-2.txt` and so on; case does not tell two names apart. */
export function foldedName(names: readonly string[]): string {
  const held = new Set(names.map((name) => name.toLowerCase()));
  if (!held.has('pasted-text.txt')) return 'pasted-text.txt';
  let next = 2;
  while (held.has(`pasted-text-${String(next)}.txt`)) next += 1;
  return `pasted-text-${String(next)}.txt`;
}

/** The toast after a paste was folded into a file (03 "Toasts"). */
export function foldedWords(name: string, bytes: number, mac: boolean): { title: string; description: string } {
  return { title: `Large paste attached as ${name}`, description: `${sizeWords(bytes)} · Use ${mac ? '⌘⇧V' : 'Ctrl+Shift+V'} to keep a large paste inline.` };
}

/** T3's `shouldHandleComposerAttachmentPaste`: an image of any sort always attaches; a plain file only when no text came with it. */
function filesAttach(clip: ClipboardFacts): boolean {
  if (clip.files.length === 0 || clip.chips) return false;
  if (clip.asText && clip.plainText.length > 0) return false;
  const picture = (file: FileFacts): boolean => isDng(file) || isSvg(file) || ['image', 'unsupported'].includes(classify(file).kind);
  return clip.files.some(picture) || clip.plainText.length === 0;
}

/**
 * 03 "Paste decision": attach the clipboard's files, fold a long text into a text file, refuse, or let the editor
 * paste. The length is counted in characters and in UTF-8 bytes, as T3 does (`pastedTextDisposition`).
 */
export function pasteDecision(clip: ClipboardFacts, draft: DraftFacts, limits: Limits = LIMITS): PasteDecision {
  if (clip.folders > 0 || filesAttach(clip)) return { act: 'attach', folders: clip.folders > 0 };
  const text = clip.plainText;
  if (clip.chips || clip.asText || text.length === 0) return TO_EDITOR;
  const start = Math.max(0, Math.min(draft.promptLength, draft.selection.start));
  const end = Math.max(start, Math.min(draft.promptLength, draft.selection.end));
  const breaksPrompt = draft.promptLength - (end - start) + text.length > limits.promptChars;
  const bytes = new TextEncoder().encode(text).byteLength;
  if (!breaksPrompt && text.length < limits.foldBytes && bytes < limits.foldBytes) return TO_EDITOR;
  if (!draft.room || limits.fileBytes === null) {
    return breaksPrompt ? { act: 'refuse', title: 'Pasted text is too large for this message', description: 'Remove some text or an attachment, then paste again.' } : TO_EDITOR;
  }
  if (bytes > limits.fileBytes) {
    return breaksPrompt ? { act: 'refuse', title: 'Pasted text is too large to attach', description: 'Reduce the clipboard contents or save a smaller excerpt as a file.' } : TO_EDITOR;
  }
  return { act: 'fold', name: foldedName(draft.names), mime: FOLDED_MIME };
}

/**
 * 03 "Layout order": the tiles of the media row, then the rows of the file list. A file that the text already shows
 * as a chip is not listed a second time (T3's `composerOtherFilesForPresentation`); a tile always shows.
 */
export function laidOut(attachments: readonly Attachment[], chipped: ReadonlySet<string> = new Set()): { media: Attachment[]; files: Attachment[] } {
  const isMedia = (attachment: Attachment): boolean => attachment.kind === 'image' || attachment.kind === 'video';
  return { media: attachments.filter(isMedia), files: attachments.filter((attachment) => !isMedia(attachment) && !chipped.has(attachment.id)) };
}

/** The attachment `by` places after the open one among those the preview dialog can show, going round at the ends. */
export function previewStep(shown: readonly Attachment[], openId: string, by: 1 | -1): string | null {
  const at = shown.findIndex((attachment) => attachment.id === openId);
  if (at < 0 || shown.length < 2) return null;
  return shown[(at + by + shown.length) % shown.length]?.id ?? null;
}
