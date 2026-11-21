import type { PaletteChat } from '$lib/code/palette/adapter';

/** The fixed moment the ages of the kit's chats are counted from, so its pictures do not change from day to day. */
export const KIT_NOW = Date.UTC(2026, 10, 21, 12);

const MINUTE = 60_000;

/** Chats of a made-up week, for the palette of the kit. */
export const KIT_CHATS: readonly PaletteChat[] = [
  { id: 'c1', title: 'Fix the sign-in redirect on the hosted hub', project: 'anima', at: KIT_NOW - 40 * MINUTE },
  { id: 'c2', title: 'Release record for the daemon', project: 'anima', at: KIT_NOW - 5 * MINUTE },
  { id: 'c3', title: 'Make the gallery pictures stable', project: 'anima', at: KIT_NOW - 26 * 60 * MINUTE },
  { id: 'c4', title: 'Pricing page copy', project: 'site', at: KIT_NOW - 3 * 24 * 60 * MINUTE },
  { id: 'c5', title: 'Windows installer gates', project: 'anima', at: KIT_NOW - 3 * 60 * MINUTE },
  { id: 'c6', title: 'Read the logs of the night run', at: KIT_NOW - 30 * 1000 },
  { id: 'c7', title: 'Release notes for 0.8', project: 'site', at: KIT_NOW - 9 * 60 * MINUTE },
];
