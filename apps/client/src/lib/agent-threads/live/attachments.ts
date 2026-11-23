/** Prompt attachments (TH111); the runner checks these limits again. */

export const MOST_IMAGE_BYTES = 10 * 1024 * 1024;
export const MOST_FILE_BYTES = 50 * 1024 * 1024;
export const MOST_ATTACHMENTS = 100;
export const MOST_IMAGE_BYTES_IN_PROMPT = 80 * 1024 * 1024;
/** The most bytes of one part, before base64. */
export const MOST_PART_BYTES = 2 * 1024 * 1024;

/** The calls of a file; `bytes` is standard base64. `size` is the bytes of the whole file. */
export type AttachAction =
  | { readonly call: 'attach'; readonly thread: string; readonly name: string; readonly mime: string; readonly size: number; readonly bytes: string }
  | { readonly call: 'attach_more'; readonly thread: string; readonly attachment: string; readonly bytes: string }
  | { readonly call: 'unattach'; readonly thread: string; readonly attachment: string };

/** A part of a file came: the file is complete when `received` is its size. */
export interface Attached {
  readonly outcome: 'attached';
  readonly id: string;
  readonly received: number;
}

export function attachedFrom(value: Readonly<Record<string, unknown>>): Attached | null {
  const { id, received } = value;
  return typeof id === 'string' && typeof received === 'number' && Number.isSafeInteger(received) ? { outcome: 'attached', id, received } : null;
}

export interface PromptFiles {
  readonly images: number;
  readonly files: readonly string[];
}

export function promptFilesFrom(value: Readonly<Record<string, unknown>>): PromptFiles {
  const images = typeof value.images === 'number' && Number.isSafeInteger(value.images) && value.images > 0 ? value.images : 0;
  const files = Array.isArray(value.files) ? value.files.filter((name): name is string => typeof name === 'string') : [];
  return { images, files };
}
