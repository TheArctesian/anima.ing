import { shortAge } from '../status/status';
import type { CodeIconName } from '../icons/icons';
import { shownActions, type PaletteChat, type PaletteHost, type ShownAction } from './adapter';

/**
 * T3 Code's limits (`CommandPalette.logic.ts`, `paletteSearch.ts`): 12 recent chats, 20 chats in a search, and 3 rows
 * of each other group while chats match.
 */
export const RECENT_LIMIT = 12;
export const CHAT_LIMIT = 20;
export const SECONDARY_LIMIT = 3;

/** One row of the palette. */
export interface PaletteRow {
  readonly id: string;
  readonly title: string;
  readonly icon: CodeIconName;
  /** The second line: a reason for a disabled action. */
  readonly detail?: string;
  /** The project of a chat, at the right. */
  readonly context?: string;
  /** "now", "5m", "3h", "2d" for a chat. */
  readonly age?: string;
  /** The command whose key the row shows. */
  readonly command?: string;
  readonly disabled: boolean;
  readonly run: () => void;
}

export interface PaletteGroup {
  readonly id: 'actions' | 'recent' | 'chats';
  readonly label: string;
  readonly rows: readonly PaletteRow[];
}

/** Folds case and accents, so "resume" finds "Résumé". */
export function folded(text: string): string {
  return text.normalize('NFD').replace(/\p{Diacritic}/gu, '').toLowerCase().trim();
}

/**
 * How well `query` names `terms` (the title first): 4 the title starts with it, 3 a word of the title does, 2 the title
 * holds it, 1 a search term does, 0 it is the first letters of the title's words ("fnw"); `null` when nothing matches.
 */
export function score(query: string, terms: readonly string[]): number | null {
  const [title = '', ...rest] = terms.map(folded);
  if (title.startsWith(query)) return 4;
  if (title.split(/[\s/._-]+/).some((word) => word.startsWith(query))) return 3;
  if (title.includes(query)) return 2;
  if (rest.some((term) => term.includes(query))) return 1;
  const initials = title.split(/[\s/._-]+/).map((word) => word.charAt(0)).join('');
  return query.length > 1 && initials.startsWith(query) ? 0 : null;
}

function actionRow(action: ShownAction): PaletteRow {
  return {
    id: `action:${action.id}`,
    title: action.label,
    icon: action.icon,
    command: action.id,
    disabled: action.run === undefined,
    ...(action.reason === undefined ? {} : { detail: action.reason }),
    run: action.run ?? (() => undefined),
  };
}

function chatRow(chat: PaletteChat, host: PaletteHost, now: number): PaletteRow {
  return {
    id: `chat:${chat.id}`,
    title: chat.title,
    icon: 'message-square',
    ...(chat.project === undefined ? {} : { context: chat.project }),
    age: shortAge(chat.at, now),
    disabled: false,
    run: () => {
      host.openChatById(chat.id);
    },
  };
}

/** The open chat goes last in its group, as T3 Code's `secondary` rows do. */
function openLast(chats: readonly PaletteChat[], open: string | undefined): PaletteChat[] {
  return [...chats.filter((chat) => chat.id !== open), ...chats.filter((chat) => chat.id === open)];
}

/** The palette with nothing typed: the actions, then the recent chats. */
function rootGroups(host: PaletteHost, actions: readonly ShownAction[], now: number): PaletteGroup[] {
  const byId = new Map(host.chats.map((chat) => [chat.id, chat]));
  const recent = host.recent.flatMap((id) => byId.get(id) ?? []).slice(0, RECENT_LIMIT);
  return [
    { id: 'actions', label: 'Actions', rows: actions.map(actionRow) },
    { id: 'recent', label: 'Recent chats', rows: openLast(recent, host.openChat).map((chat) => chatRow(chat, host, now)) },
  ];
}

function ranked<T>(items: readonly T[], query: string, terms: (item: T) => readonly string[]): { item: T; score: number }[] {
  return items
    .flatMap((item) => {
      const found = score(query, terms(item));
      return found === null ? [] : [{ item, score: found }];
    })
    .toSorted((left, right) => right.score - left.score);
}

/**
 * The groups for what the person typed. A `>` first lists the actions alone (T3 Code). With text, the actions and
 * the chats are ranked by `score`, chats that tie newest first; a group whose best row matches better comes first,
 * actions first on a tie.
 * Empty groups are left out.
 */
export function paletteGroups(host: PaletteHost, typed: string, now: number): PaletteGroup[] {
  const actionsOnly = typed.startsWith('>');
  const query = folded(actionsOnly ? typed.slice(1) : typed);
  const actions = shownActions(host);
  if (query === '') {
    const groups = rootGroups(host, actions, now);
    return (actionsOnly ? groups.slice(0, 1) : groups).filter((group) => group.rows.length > 0);
  }
  // Newest first, so chats that match equally well keep that order (the sort is stable).
  const newest = host.chats.toSorted((left, right) => right.at - left.at);
  const foundChats = actionsOnly ? [] : ranked(newest, query, (chat) => [chat.title, chat.project ?? '']).slice(0, CHAT_LIMIT);
  const allActions = ranked(actions, query, (action) => [action.label, ...action.terms]);
  const foundActions = foundChats.length > 0 ? allActions.slice(0, SECONDARY_LIMIT) : allActions;
  // The open chat is a `secondary` row: it shows, but it never lifts its group above the others.
  const chatsBest = foundChats.find(({ item }) => item.id !== host.openChat)?.score ?? (foundChats.length > 0 ? 0 : -1);
  const groups = [
    { best: foundActions[0]?.score ?? -1, group: { id: 'actions', label: 'Actions', rows: foundActions.map(({ item }) => actionRow(item)) } },
    {
      best: chatsBest,
      group: { id: 'chats', label: 'Chats', rows: openLast(foundChats.map(({ item }) => item), host.openChat).map((chat) => chatRow(chat, host, now)) },
    },
  ] as const;
  return groups
    .filter(({ group }) => group.rows.length > 0)
    .toSorted((left, right) => right.best - left.best)
    .map(({ group }) => group);
}

/** The words of an empty palette: T3 Code's, for what Code searches. */
export function emptyWords(typed: string): string {
  return typed.startsWith('>') ? 'No matching actions.' : 'No matching commands or chats.';
}

/** The row after a move of `by` from `active` among `count` rows, round from the last to the first. */
export function stepped(active: number, by: 1 | -1, count: number): number {
  if (count === 0) return -1;
  if (active < 0) return by === 1 ? 0 : count - 1;
  return (active + by + count) % count;
}
