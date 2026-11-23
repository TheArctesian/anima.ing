import type { Violation } from "../report.ts";

export function goFiles(paths: string[]): string[] {
  return paths.filter((path) => path.endsWith(".go"));
}

export function unformatted(gofmtOutput: string): Violation[] {
  return gofmtOutput
    .split("\n")
    .filter((path) => path !== "")
    .map((path) => ({ path, signal: "gofmt", detail: "run `gofmt -w` on this file" }));
}
