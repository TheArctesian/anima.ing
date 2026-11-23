/** `trial-bundle.sh` sets these; a release build sets none. */
interface BuildValues {
  readonly VITE_ANIMA_LOCAL_BUILD?: string;
}

/** What the Settings row says in a local build, in place of the update state (UPD-006, DEF-001). */
export const LOCAL_BUILD_LINE = 'This is a local build. It does not update itself.';

/** A trial build never checks the feed: the updater would install a lower published version (UPD-001, DEF-001). */
export function isLocalBuild(values: BuildValues = import.meta.env as BuildValues): boolean {
  return values.VITE_ANIMA_LOCAL_BUILD === '1';
}
