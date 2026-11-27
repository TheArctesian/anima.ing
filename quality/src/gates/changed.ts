import { OS_SOURCE } from "./os.ts";
import type { Gate } from "./gates.ts";
import { workspaceMaps, type Choice, type Layout, type Member } from "./tier.ts";

export interface ChangedRun {
  gates: Gate[];
  /** What the run leaves to GitHub, one line each. */
  notes: string[];
}

export function dependents(names: string[], members: Member[]): string[] {
  const found = new Set(names);
  let grew = true;
  while (grew) {
    grew = false;
    for (const member of members) {
      if (!found.has(member.name) && (member.uses ?? []).some((each) => found.has(each))) {
        found.add(member.name);
        grew = true;
      }
    }
  }
  return members.map((member) => member.name).filter((name) => found.has(name) && !names.includes(name));
}

const flags = (option: string, names: string[]) => names.flatMap((name) => [option, name]);

export function owners(paths: string[], members: Member[]): string[] {
  const found = new Set<string>();
  for (const path of paths) {
    const owner = members.filter((member) => path.startsWith(`${member.folder}/`)).sort((one, other) => other.folder.length - one.folder.length)[0];
    if (owner !== undefined) found.add(owner.name);
  }
  return members.map((member) => member.name).filter((name) => found.has(name));
}

function named(full: Gate[], ...names: string[]): Gate[] {
  return full.filter((gate) => names.includes(gate.name));
}

/** `own` holds the members whose own files changed; only their tests run when `touched` is "all". */
interface Touch {
  touched: "all" | string[];
  own: string[];
}

function rustGates(full: Gate[], { touched, own }: Touch, crates: Member[], notes: string[]): Gate[] {
  const all = touched === "all";
  const tested = all ? own : touched;
  if (tested.length === 0 && !all) return [];
  const users = all ? [] : dependents(tested, crates);
  if (all) notes.push(`each Rust package is touched: Clippy of the workspace runs here; only the tests of ${tested.join(", ") || "no crate"} run here, the others on GitHub`);
  if (users.length > 0) notes.push(`Clippy also checks the crates that use them (${users.join(", ")}); their tests run on GitHub`);
  const test = named(full, "rust-test")[0];
  return [
    ...named(full, "fmt"),
    ...(all ? named(full, "clippy") : [{ name: "clippy", group: "rust" as const, argv: ["cargo", "clippy", ...flags("-p", [...tested, ...users]), "--all-targets", "--", "-D", "warnings"] }]),
    ...(tested.length === 0
      ? []
      : [{
          name: "rust-test",
          group: "rust" as const,
          argv: ["bun", "quality/bin/with-postgres.ts", "cargo", "nextest", "run", ...flags("-p", tested), "--no-tests=pass"],
          requires: test?.requires,
        }]),
  ];
}

function bunGates(full: Gate[], { touched, own }: Touch, packages: Member[], notes: string[]): Gate[] {
  const all = touched === "all";
  const tested = all ? own : touched;
  if (tested.length === 0 && !all) return [];
  const users = all ? [] : dependents(tested, packages);
  if (all) notes.push(`each bun package is touched: lint and the check of types run for each here; only the tests of ${tested.join(", ") || "no package"} run here, the others and the build on GitHub`);
  if (users.length > 0) notes.push(`lint and the check of types also run for the packages that use them (${users.join(", ")}); their tests run on GitHub`);
  const script = (name: string, names: string[]): Gate => ({ name, group: "bun", argv: ["bun", "run", ...flags("--filter", names), name] });
  return [
    ...named(full, "install"),
    ...(all ? named(full, "lint", "check") : [script("lint", [...tested, ...users]), script("check", [...tested, ...users])]),
    ...(tested.length === 0 ? [] : [script("test", tested)]),
    ...(!all && tested.includes("@anima/client") ? named(full, "build") : []),
  ];
}

export function changedGates(choice: Choice, full: Gate[], layout: Layout, paths: string[] = []): ChangedRun {
  const documents = named(full, "documents");
  const notes: string[] = [];
  const quality = full.filter((gate) => gate.group === "quality" && gate.name !== "documents");
  switch (choice.tier) {
    case "nothing":
    case "documents":
      return { gates: documents, notes };
    case "full":
      notes.push(choice.os ? "OS source and quality gates run here; Linux controller and installed-image qualification remain separate" : "a path that no rule knows: only the quality gates run here, the full list runs on GitHub");
      return { gates: [...documents, ...quality, ...(choice.os ? [OS_SOURCE] : [])], notes };
    case "code": {
      const bunOwn = owners(paths, layout.packages);
      const deployTested = (choice.bun === "all" ? bunOwn : choice.bun).includes("@anima/deploy");
      return {
        gates: [
          ...documents,
          ...rustGates(full, { touched: choice.rust, own: owners(paths, layout.crates) }, layout.crates, notes),
          ...(choice.go ? full.filter((gate) => gate.group === "go") : []),
          ...bunGates(full, { touched: choice.bun, own: bunOwn }, layout.packages, notes),
          ...workspaceMaps(choice.rust, deployTested),
          ...quality,
          ...(choice.os ? [OS_SOURCE] : []),
        ],
        notes,
      };
    }
  }
}
