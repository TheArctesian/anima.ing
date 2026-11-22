import { isTestFile } from './tests-rule';
import type { ChangedFile } from './patch';

/**
 * How the diff surface shows a diff (t3-ui-copy-2026-11-21/05, Section 1.5): the choices of the right group of its
 * header. The owner of the surface keeps them; the defaults are those of T3 Code.
 */
export interface DiffView {
  /** Two columns; stacked when off. */
  readonly split: boolean;
  readonly wrap: boolean;
  /** Changes of white space only are left out; the source of the diff does the work. */
  readonly whitespaceHidden: boolean;
  readonly testsHidden: boolean;
  readonly tree: boolean;
}

export const DEFAULT_VIEW: DiffView = { split: false, wrap: false, whitespaceHidden: true, testsHidden: false, tree: false };

/** The files that show: with "Hide test files" on, a test file shows only when it was asked for by name. */
export function shownFiles(files: readonly ChangedFile[], view: DiffView, revealed: ReadonlySet<string> = new Set()): ChangedFile[] {
  if (!view.testsHidden) return [...files];
  return files.filter((file) => revealed.has(file.path) || !isTestFile(file.path));
}

export function testCount(files: readonly ChangedFile[]): number {
  return files.filter((file) => isTestFile(file.path)).length;
}

/** The last part of a path, and the folders before it. */
export function split(path: string): { readonly name: string; readonly folder: string } {
  const at = path.lastIndexOf('/');
  return at === -1 ? { name: path, folder: '' } : { name: path.slice(at + 1), folder: path.slice(0, at) };
}
