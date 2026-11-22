/**
 * The files that go with a prompt (TH111): the calls that send a file to the runner in parts, its answer, and the limits
 * that the runner checks again. A file over the frame goes in parts of at most `MOST_PART_BYTES` before base64, in order.
 */

/** The most bytes of one image (png, jpeg, gif, webp). */
export const MOST_IMAGE_BYTES = 10 * 1024 * 1024;
/** The most bytes of one file that is not an image. */
export const MOST_FILE_BYTES = 50 * 1024 * 1024;
/** The most files with one prompt. */
export const MOST_ATTACHMENTS = 100;
/** The most bytes of the images of one prompt together. */
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

/** The answer `attached`, or `null` when it does not read. */
export function attachedFrom(value: Readonly<Record<string, unknown>>): Attached | null {
  const { id, received } = value;
  return typeof id === 'string' && typeof received === 'number' && Number.isSafeInteger(received) ? { outcome: 'attached', id, received } : null;
}

/** What went with a prompt, as its `prompt` change says: how many images, and the names of the other files. */
export interface PromptFiles {
  readonly images: number;
  readonly files: readonly string[];
}

/** The files of a `prompt` change; none when the change names none. */
export function promptFilesFrom(value: Readonly<Record<string, unknown>>): PromptFiles {
  const images = typeof value.images === 'number' && Number.isSafeInteger(value.images) && value.images > 0 ? value.images : 0;
  const files = Array.isArray(value.files) ? value.files.filter((name): name is string => typeof name === 'string') : [];
  return { images, files };
}
