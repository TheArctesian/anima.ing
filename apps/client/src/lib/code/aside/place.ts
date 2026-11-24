import type { RunRow } from '$lib/agent-threads/live/run';
import type { Details } from '../details/model';
import { shows, type ShownChoices, type ShownId } from '../details/shown';

/**
 * The card of a thread's details beside the chat (T3 Code `threadDetailsCardLayout.ts`, `chatCanvasLayout.ts`): 280 px
 * wide, 12 px in from the canvas edges, inline when the lane keeps 640 px with 48 px padding and 32 px clear of the card,
 * and a popover from the details toggle when it does not. Pure.
 */
export const CARD = { width: 280, gap: 12, clearance: 32, padding: 48, minChat: 640, minHeight: 160 } as const;

/**
 * What the card shows of a run, from what the runner knows of it (TH112): the workspace, the branch and the context
 * window. A part that the runner does not know is absent, so the card draws nothing false: a folder with no Git has no
 * workspace row, since the card's words for it would say "Local checkout".
 */
export function detailsOf(row: RunRow): Details {
  const workspace = row.workspace ?? null;
  const context = row.usage?.context ?? null;
  const max = row.usage?.window ?? null;
  return {
    ...(workspace === null || workspace.kind === 'folder'
      ? {}
      : { runContext: { environments: [], mode: 'local', path: row.folder, locked: true, ...(workspace.kind === 'worktree' ? { worktreePath: workspace.top } : {}) } }),
    ...(workspace?.branch == null ? {} : { branch: { name: workspace.branch } }),
    ...(context === null ? {} : { context: { used: context, ...(max === null ? {} : { max }) } }),
  };
}

/** The item of the "Choose what to show" menu that shows each part `detailsOf` can give. */
const PART_ITEM = { runContext: 'environment', branch: 'branch', context: 'context' } as const satisfies Partial<Record<keyof Details, ShownId>>;

/**
 * The run of the open thread `chosen`, when the card has something of it to show by the person's `choices`; otherwise
 * none, and no toggle. A run whose only part is hidden by default (the context window of a scratch folder) has no card:
 * an empty card would stand over the chat with only "Choose what to show" in it. A part that the person hid keeps the
 * card, so its menu can show the part again (T3 keeps an emptied card for that).
 */
export function detailedRun(row: RunRow | undefined, chosen: string | null, choices: ShownChoices = {}): RunRow | undefined {
  if (row === undefined || row.id !== chosen) return undefined;
  const items = (Object.keys(detailsOf(row)) as (keyof typeof PART_ITEM)[]).map((part) => PART_ITEM[part]);
  return items.some((item) => shows(choices, item) || choices[item] === false) ? row : undefined;
}

/** The card stands inline in a canvas of this size; otherwise it opens as a popover. */
export function fitsInline(canvas: { readonly width: number; readonly height: number }): boolean {
  const lane = canvas.width - CARD.width - CARD.gap - CARD.clearance - CARD.padding;
  return lane >= CARD.minChat && canvas.height - CARD.gap * 2 >= CARD.minHeight;
}

/**
 * The room the lanes leave at their end while the card stands inline, so the timeline and the composer end 32 px before
 * the card (T3 moves the lane, not the card). Each lane is centred in what is left, with 48 px at its sides and at most
 * `laneMax` wide (the person's chat width): its end is the nearer of the centred one and the one held by its padding.
 */
export function laneRoom(width: number, laneMax: number): number {
  const laneRight = width - CARD.gap - CARD.width - CARD.clearance;
  const centred = width - 2 * laneRight + laneMax;
  const padded = width - CARD.padding - laneRight;
  return Math.max(0, Math.round(Math.min(centred, padded)));
}
