import { isDocument } from "./tier.ts";

export interface Jobs {
  rust: boolean;
  rustCrates?: string[];
  go: boolean;
  documentsOnly: boolean;
}

export const EACH_JOB: Jobs = { rust: true, go: true, documentsOnly: false };

const RUNNERS = [
  ".github/",
  "quality/bin/gates.ts",
  "quality/src/gates/",
  "quality/bin/with-postgres.ts",
  "quality/bin/go-format.ts",
  "quality/bin/rustc-cache.sh",
];

// Rust tests read `link/hublink/`, `deploy/postgres/roles.sql` and `contracts/`; the theme crate reads `design/tokens`.
// The desktop shell embeds `apps/client/build`, but no Rust job builds the client.
const NOT_RUST = ["apps/client/", "apps/desktop/", "link/", "deploy/", "quality/"];
const RUST_INSIDE = ["apps/desktop/src-tauri/", "link/hublink/", "deploy/postgres/"];

function readByRust(path: string): boolean {
  if (RUST_INSIDE.some((prefix) => path.startsWith(prefix))) return true;
  return !NOT_RUST.some((prefix) => path.startsWith(prefix));
}

export type TestOwner = (path: string) => string | undefined;

export function jobsFor(paths: string[], owner: TestOwner = () => undefined): Jobs {
  if (paths.some((path) => RUNNERS.some((runner) => path.startsWith(runner)))) return EACH_JOB;
  const code = paths.filter((path) => !isDocument(path));
  if (code.length === 0) return { rust: false, go: false, documentsOnly: true };
  const rust = code.filter(readByRust);
  const owners = rust.map(owner);
  const jobs = { rust: rust.length > 0, go: code.some((path) => path.startsWith("link/")), documentsOnly: false };
  if (rust.length === 0 || owners.some((crate) => crate === undefined)) return jobs;
  return { ...jobs, rustCrates: [...new Set(owners as string[])].sort() };
}

/** `proved` maps each tree whose gates passed as it is to the number of that run. */
export interface DevPush {
  tree: string;
  before?: { tree: string; changed: string[] };
  proved: ReadonlyMap<string, number>;
}

export interface DevJobs {
  jobs: Jobs;
  provedBy?: number;
  reason: string;
}

export function devPushJobs({ tree, before, proved }: DevPush): DevJobs {
  const same = proved.get(tree);
  if (same !== undefined) return { jobs: { rust: false, go: false, documentsOnly: true }, provedBy: same, reason: `run ${String(same)} passed for this tree` };
  const parent = before === undefined ? undefined : proved.get(before.tree);
  const documents = before !== undefined && before.changed.length > 0 && before.changed.every(isDocument);
  if (parent !== undefined && documents) {
    return { jobs: { rust: false, go: false, documentsOnly: true }, reason: `only documents changed on the tree that run ${String(parent)} proved` };
  }
  return { jobs: EACH_JOB, reason: parent === undefined ? "no run proved the tree before this push" : "the push changed more than documents" };
}

export function jobLines(jobs: Jobs, proved?: number): string[] {
  const lines = [`rust=${String(jobs.rust)}`, `go=${String(jobs.go)}`, `documents_only=${String(jobs.documentsOnly)}`, `rust_crates=${jobs.rustCrates?.join(",") ?? ""}`];
  return proved === undefined ? lines : [...lines, `proved=${String(proved)}`];
}
