// Lints `cfg(windows)` code on Linux: Clippy links nothing, so the GNU target needs only mingw-w64.
// No crate asks for `target_env = "msvc"`, so the GNU code is the code of the Windows build.

export const WINDOWS_TARGET = "x86_64-pc-windows-gnu";

export interface LeftOut {
  crate: string;
  reason: string;
}

export const LEFT_OUT: readonly LeftOut[] = [];

/** `--keep-going`: a fault in one crate does not stop the check of the crates that do not depend on it. */
export function windowsLintArgv(leftOut: readonly LeftOut[] = LEFT_OUT): string[] {
  return [
    "cargo",
    "clippy",
    "--workspace",
    ...leftOut.flatMap(({ crate }) => ["--exclude", crate]),
    "--all-targets",
    "--target",
    WINDOWS_TARGET,
    "--locked",
    "--keep-going",
    "--",
    "-D",
    "warnings",
  ];
}
