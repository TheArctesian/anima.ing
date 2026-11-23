import { delimiter, join } from "node:path";

export interface CacheChoice {
  env: Record<string, string | undefined>;
  note: string;
}

/** sccache cannot cache incremental builds, so incremental compilation is off. */
export function compileCache(env: Record<string, string | undefined>, exists: (path: string) => boolean): CacheChoice {
  if (env["RUSTC_WRAPPER"] !== undefined) {
    return { env, note: `rust cache: RUSTC_WRAPPER is set (${env["RUSTC_WRAPPER"] || "empty"}), kept as it is` };
  }
  const dirs = (env["PATH"] ?? env["Path"] ?? "").split(delimiter).filter((dir) => dir !== "");
  const folder = dirs.find((dir) => exists(join(dir, process.platform === "win32" ? "sccache.exe" : "sccache")));
  if (folder === undefined) {
    return { env: process.platform === "win32" ? { ...env, RUSTC_WRAPPER: "" } : env, note: "rust cache: off (install sccache to share Rust builds between worktrees: brew install sccache)" };
  }
  const wrapper = join(folder, process.platform === "win32" ? "sccache.exe" : "sccache");
  return { env: { ...env, RUSTC_WRAPPER: wrapper, CARGO_INCREMENTAL: "0" }, note: `rust cache: sccache (${wrapper})` };
}
