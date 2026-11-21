import type { CodeIconName } from '../../icons/icons';
import type { Chip, ChipReader } from '../draft';
import type { Skill } from './sources';

/**
 * The mention tokens of a prompt, as the source stores them in the Markdown (03 "What a choice inserts" and "The
 * chosen item as an inline chip"; T3 Code `packages/shared/src/composerInlineTokens.ts` and
 * `composerContextReferences.ts`):
 *
 * - a file or folder: the link `[base name](path)` whose words are the base name of its path, or the older `@path`
 *   and `@"path with spaces"`;
 * - a skill: `$name`;
 * - a thread: the link `[title](anima-context://v1/thread/<id>)`.
 *
 * A file or a skill is a chip only between white space (or the prompt's start) and white space, so a name that is
 * still typed is not one. A thread link is a chip wherever it stands.
 */
export type ChipKind = 'mention' | 'skill' | 'thread';

export interface MentionChip extends Chip {
  readonly kind: ChipKind;
}

/** What the reader must know to name a chip: a `$name` the agent does not offer stays plain text (03, last lines of the chip section). */
export interface KnownSkills {
  readonly skills: readonly Skill[];
}

const LABEL_MAX = 512;
const FILE_LINK = new RegExp(String.raw`(^|\s)\[((?:\\.|[^\]\\]){0,${String(LABEL_MAX)}})\]\(([^)\s]+)\)(?=\s)`, 'g');
const BARE_MENTION = /(^|\s)@(?:"((?:\\.|[^"\\])*)"|([^\s@"]+))(?=\s)/g;
const SKILL =
  /(^|\s)\p{Sc}(?![0-9][0-9_]*(?:[kKmMbBtT]|[eE][0-9]+)?(?:\s|$))(?=[a-zA-Z0-9:_-]*[a-zA-Z])([a-zA-Z0-9][a-zA-Z0-9:_-]*)(?=\s)/gu;
const CONTEXT_PREFIX = 'anima-context://v1/';
const CONTEXT_LINK = new RegExp(String.raw`\[([^[\]\n]{0,${String(LABEL_MAX)}})\]\((${CONTEXT_PREFIX}[^\s)]{1,200})\)`, 'g');
const CONTEXT_ID = /^[a-z0-9_-]{1,128}$/i;
const URI_SCHEME = /^[A-Za-z][A-Za-z0-9+.-]*:/;
const WINDOWS_DRIVE = /^[A-Za-z]:[\\/]/;
/** `@scope/package` and `@decorator(` are code, not files. */
const SCOPED_PACKAGE = /^[a-z0-9][a-z0-9._-]*\/[a-z0-9][a-z0-9._-]*(?:\/[^\s@"]+)*$/;
const DECORATOR_CALL = /^[\p{ID_Start}_$][\p{ID_Continue}.$\u200c\u200d]*\(/u;

const IMAGE = /\.(?:png|jpe?g|gif|webp|avif|bmp|ico|svg)$/i;
const TEXT = /\.(?:md|mdx|txt|rst|adoc|log)$/i;
const CODE =
  /\.(?:ts|tsx|js|jsx|mjs|cjs|svelte|vue|rs|go|py|rb|java|kt|swift|c|h|cc|cpp|hpp|cs|php|sh|zsh|bash|sql|css|scss|html|json|ya?ml|toml|graphql|nix|lua)$/i;

export function baseName(path: string): string {
  const separator = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return separator >= 0 ? path.slice(separator + 1) : path;
}

/** The icon of a path. T3's file-type icon set is not copied: five of our icons stand for it. */
export function pathIcon(path: string, folder = path.endsWith('/')): CodeIconName {
  if (folder) return 'folder';
  if (IMAGE.test(path)) return 'image';
  if (TEXT.test(path)) return 'file-text';
  return CODE.test(path) ? 'file-code' : 'file';
}

/** The link a chosen file is written as. The words escape `\ [ ] * _` so that `__init__.py` is not read as bold. */
export function fileLink(path: string): string {
  const label = baseName(path).replace(/[\\[\]*_]/g, String.raw`\$&`);
  const destination = encodeURI(path).replaceAll('(', '%28').replaceAll(')', '%29').replaceAll('#', '%23').replaceAll('?', '%3F').replaceAll('\\', '%5C');
  return `[${label}](${destination})`;
}

/** The link a chosen thread is written as; null when its id cannot stand in a link. */
export function threadLink(thread: { readonly id: string; readonly title: string }): string | null {
  if (!CONTEXT_ID.test(thread.id)) return null;
  const label = thread.title.replace(/[[\]\\\r\n]/g, ' ').replace(/\s+/g, ' ').trim();
  return `[${label === '' ? 'thread' : label}](${CONTEXT_PREFIX}thread/${thread.id})`;
}

function decoded(path: string): string {
  try {
    return decodeURIComponent(path);
  } catch {
    // A stray `%` is not an encoding: the path stands as written, as in the source.
    return path;
  }
}

function fileChip(path: string, start: number, end: number): MentionChip {
  return { kind: 'mention', start, end, label: baseName(path), icon: pathIcon(path), detail: path };
}

function fileLinks(prompt: string): MentionChip[] {
  const chips: MentionChip[] = [];
  for (const match of prompt.matchAll(FILE_LINK)) {
    const path = decoded(match[3] ?? '');
    const label = (match[2] ?? '').replace(/\\(.)/g, '$1');
    const external = URI_SCHEME.test(path) && !WINDOWS_DRIVE.test(path);
    if (path === '' || external || label !== baseName(path)) continue;
    const start = match.index + (match[1] ?? '').length;
    chips.push(fileChip(path, start, match.index + match[0].length));
  }
  return chips;
}

function bareMentions(prompt: string): MentionChip[] {
  const chips: MentionChip[] = [];
  for (const match of prompt.matchAll(BARE_MENTION)) {
    const quoted = match[2];
    const path = quoted === undefined ? (match[3] ?? '') : quoted.replace(/\\(.)/g, '$1');
    if (path === '' || (quoted === undefined && (SCOPED_PACKAGE.test(path) || DECORATOR_CALL.test(path)))) continue;
    const start = match.index + (match[1] ?? '').length;
    chips.push(fileChip(path, start, match.index + match[0].length));
  }
  return chips;
}

function skillChips(prompt: string, known: KnownSkills | undefined): MentionChip[] {
  const chips: MentionChip[] = [];
  for (const match of prompt.matchAll(SKILL)) {
    const name = match[2] ?? '';
    const skill = known?.skills.find((each) => each.name === name);
    if (known !== undefined && skill === undefined) continue;
    const start = match.index + (match[1] ?? '').length;
    const label = skill?.displayName?.trim() ?? '';
    const detail = skill?.shortDescription ?? skill?.description ?? `Skill ${name}`;
    chips.push({ kind: 'skill', start, end: match.index + match[0].length, label: label === '' ? name : label, icon: 'sparkles', detail });
  }
  return chips;
}

function threadChips(prompt: string): MentionChip[] {
  const chips: MentionChip[] = [];
  for (const match of prompt.matchAll(CONTEXT_LINK)) {
    const [kind, id, ...more] = (match[2] ?? '').slice(CONTEXT_PREFIX.length).split('/');
    if (kind !== 'thread' || id === undefined || more.length > 0 || !CONTEXT_ID.test(id)) continue;
    const label = match[1] ?? '';
    chips.push({ kind: 'thread', start: match.index, end: match.index + match[0].length, label, icon: 'message-circle-question', detail: `Thread ${label}` });
  }
  return chips;
}

function overlaps(a: MentionChip, b: MentionChip): boolean {
  return a.start < b.end && a.end > b.start;
}

function chipsOf(prompt: string, known: KnownSkills | undefined): MentionChip[] {
  const threads = threadChips(prompt);
  // A half-typed `@` mention must not take the start of a thread link's words.
  const others = [...fileLinks(prompt), ...bareMentions(prompt), ...skillChips(prompt, known)].filter((chip) => !threads.some((thread) => overlaps(chip, thread)));
  const chips: MentionChip[] = [];
  for (const chip of [...others, ...threads].sort((left, right) => left.start - right.start)) {
    if (chip.start >= (chips.at(-1)?.end ?? 0)) chips.push(chip);
  }
  return chips;
}

/** Every mention token of a prompt, in order. Each `$name` is a chip: use `chipReader` where the skills are known. */
export function readChips(prompt: string): readonly MentionChip[] {
  return chipsOf(prompt, undefined);
}

/** A reader that draws only the skills the agent offers, by their display names. */
export function chipReader(known: KnownSkills): ChipReader & ((prompt: string) => readonly MentionChip[]) {
  return (prompt) => chipsOf(prompt, known);
}
