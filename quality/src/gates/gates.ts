import { delimiter, join } from "node:path";

export interface Requirement {
  executable: string;
  install: string;
  folder: string;
}

export type Group = "rust" | "go" | "bun" | "quality";
export const GROUPS: readonly Group[] = ["rust", "go", "bun", "quality"];

export interface Gate {
  name: string;
  group: Group;
  argv: string[];
  requires?: Requirement[];
}

/** `code` is 1 when a signal killed the gate. */
export type GateEvent =
  | { kind: "start"; position: number; total: number; gate: Gate }
  | { kind: "pass"; position: number; total: number; gate: Gate; seconds: number }
  | { kind: "fail"; position: number; total: number; gate: Gate; code: number; message?: string }
  | { kind: "done"; passed: number; total: number; stoppedAt?: Gate; failed?: Gate[] };

export interface GateOutcome {
  exitCode: number;
  stoppedAt?: Gate;
  failed?: Gate[];
}

/** `keepGoing` is for a proof run only. */
export interface GateRun {
  now?: () => number;
  keepGoing?: boolean;
}

export type RunProcess = (gate: Gate) => number;
export type ReportGateEvent = (event: GateEvent) => void;

const NEXTEST: Requirement = {
  executable: "cargo-nextest",
  install: "cargo install cargo-nextest --locked",
  folder: "~/.cargo/bin",
};
const GO: Requirement = { executable: "go", install: "brew install go", folder: "/opt/homebrew/bin, or /usr/local/go/bin" };
const GOFMT: Requirement = { ...GO, executable: "gofmt" };

const GO_MODULE = "link";

function goGate(name: string, ...args: string[]): Gate {
  return { name, group: "go", argv: ["go", "-C", GO_MODULE, ...args], requires: [GO] };
}

/** CQ-010, CQ-011. Gates 3 to 5 together run every test that `cargo test` runs (R-112). */
export function gateList(baseRef: string): Gate[] {
  return [
    { name: "fmt", group: "rust", argv: ["cargo", "fmt", "--check"] },
    { name: "clippy", group: "rust", argv: ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"] },
    // Rust tests run against a throwaway PostgreSQL unless DATABASE_URL is set (R-009).
    {
      name: "rust-test",
      group: "rust",
      argv: ["bun", "quality/bin/with-postgres.ts", "cargo", "nextest", "run", "--workspace"],
      requires: [NEXTEST],
    },
    { name: "doctest", group: "rust", argv: ["cargo", "test", "--workspace", "--doc"] },
    // `harness = false`, so not listable by nextest (.config/nextest.toml); `--workspace` keeps gate 3's build.
    { name: "pinning", group: "rust", argv: ["cargo", "test", "--workspace", "--test", "pinning"] },
    { name: "go-fmt", group: "go", argv: ["bun", "quality/bin/go-format.ts"], requires: [GOFMT] },
    goGate("go-vet", "vet", "./..."),
    goGate("go-staticcheck", "tool", "staticcheck", "./..."),
    goGate("go-revive", "tool", "revive", "-config", "revive.toml", "-set_exit_status", "./..."),
    goGate("go-test", "test", "-race", "./..."),
    // `-o /dev/null`: with one main package, a plain `go build` writes the program into `link/`.
    goGate("go-build", "build", "-o", process.platform === "win32" ? "NUL" : "/dev/null", "./..."),
    { name: "install", group: "bun", argv: ["bun", "install", "--frozen-lockfile"] },
    { name: "lint", group: "bun", argv: ["bun", "run", "lint"] },
    { name: "check", group: "bun", argv: ["bun", "run", "check"] },
    { name: "test", group: "bun", argv: ["bun", "run", "test"] },
    { name: "build", group: "bun", argv: ["bun", "run", "--cwd", "apps/client", "build"] },
    { name: "limits", group: "quality", argv: ["bun", "run", "quality:limits"] },
    { name: "names", group: "quality", argv: ["bun", "run", "quality:names"] },
    { name: "imports", group: "quality", argv: ["bun", "run", "quality:imports"] },
    { name: "documents", group: "quality", argv: ["bun", "run", "quality:documents"] },
    { name: "baseline", group: "quality", argv: ["bun", "run", "quality:baseline", "--", "--base", baseRef] },
  ];
}

export function ofGroups(gates: Gate[], groups: readonly Group[]): Gate[] {
  return gates.filter((gate) => groups.includes(gate.group));
}

/** A runner that throws is a failure, never a skip (CQ-024). */
function runOne(gate: Gate, run: RunProcess): { code: number; message?: string } {
  try {
    return { code: run(gate) };
  } catch (error) {
    return { code: 1, message: error instanceof Error ? error.message : String(error) };
  }
}

export function runGates(gates: Gate[], run: RunProcess, report: ReportGateEvent, { now = Date.now, keepGoing = false }: GateRun = {}): GateOutcome {
  const total = gates.length;
  const failed: Gate[] = [];
  let exitCode = 0;
  for (const [index, gate] of gates.entries()) {
    const position = index + 1;
    report({ kind: "start", position, total, gate });
    const began = now();
    const { code, message } = runOne(gate, run);
    if (code === 0) {
      report({ kind: "pass", position, total, gate, seconds: (now() - began) / 1000 });
      continue;
    }
    report(message === undefined ? { kind: "fail", position, total, gate, code } : { kind: "fail", position, total, gate, code, message });
    if (!keepGoing) {
      report({ kind: "done", passed: index, total, stoppedAt: gate });
      return { exitCode: code, stoppedAt: gate };
    }
    failed.push(gate);
    if (exitCode === 0) exitCode = code;
  }
  if (failed.length === 0) {
    report({ kind: "done", passed: total, total });
    return { exitCode: 0 };
  }
  report({ kind: "done", passed: total - failed.length, total, failed });
  return { exitCode, failed };
}

export function keptGoing(gates: Gate[]): Gate[] {
  return gates.map((gate) => {
    if (gate.name === "clippy") {
      const at = gate.argv.indexOf("--");
      return { ...gate, argv: [...gate.argv.slice(0, at), "--keep-going", ...gate.argv.slice(at)] };
    }
    if (gate.name === "rust-test" || gate.name === "doctest" || gate.name === "pinning") return { ...gate, argv: [...gate.argv, "--no-fail-fast"] };
    return gate;
  });
}

/** A missing tool fails its gate, never skips it (CQ-024). */
export function missingRequirement(gate: Gate, pathEnv: string, exists: (path: string) => boolean): string | undefined {
  const dirs = pathEnv.split(delimiter).filter((dir) => dir !== "");
  const missing = (gate.requires ?? []).find((need) => !dirs.some((dir) => exists(join(dir, process.platform === "win32" ? `${need.executable}.exe` : need.executable))));
  if (missing === undefined) return undefined;
  return `${missing.executable} is not on PATH; install it with \`${missing.install}\` and put its folder (${missing.folder}) on PATH`;
}

export function formatGateEvent(event: GateEvent): string {
  switch (event.kind) {
    case "start":
      return `[${event.position}/${event.total}] ${event.gate.name}: ${event.gate.argv.join(" ")}`;
    case "pass":
      return `[${event.position}/${event.total}] ${event.gate.name}: pass (${event.seconds.toFixed(1)}s)`;
    case "fail":
      return `[${event.position}/${event.total}] ${event.gate.name}: FAIL (exit ${event.code})${event.message ? ` ${event.message}` : ""}`;
    case "done":
      if (event.stoppedAt) return `gates: stopped at ${event.stoppedAt.name}`;
      if (event.failed) return `gates: ${event.passed}/${event.total} pass; failed: ${event.failed.map((gate) => gate.name).join(", ")}`;
      return `gates: ${event.passed}/${event.total} pass`;
  }
}
