import { invoke, isTauri } from '@tauri-apps/api/core';

export interface Access {
  readonly configured: boolean;
  readonly desktop: boolean;
  readonly username: string;
  readonly selected: readonly number[];
  readonly open: readonly number[];
  readonly notes: readonly { readonly port: number; readonly message: string }[];
}

const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null;
const ports = (value: unknown): value is number[] => Array.isArray(value) && value.every((port) => [22, 3389, 5900].includes(port as number));

export async function localAccess(selected?: readonly number[]): Promise<Access | null> {
  if (!isTauri()) return null;
  const value: unknown = await invoke('link_access', selected === undefined ? {} : { ports: selected });
  if (!record(value) || typeof value.configured !== 'boolean' || typeof value.desktop !== 'boolean' || typeof value.username !== 'string' || !ports(value.selected) || !ports(value.open) || !Array.isArray(value.notes)) throw new Error('The app could not read remote access settings.');
  const notes: { port: number; message: string }[] = [];
  for (const note of value.notes) {
    if (!record(note) || typeof note.port !== 'number' || typeof note.message !== 'string') throw new Error('The app could not read setup progress.');
    notes.push({ port: note.port, message: note.message });
  }
  return { configured: value.configured, desktop: value.desktop, username: value.username, selected: value.selected, open: value.open, notes };
}
