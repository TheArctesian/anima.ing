import type { Gate } from "./gates.ts";
import type { Member } from "./tier.ts";

// A change that only the tests of one crate read cannot change what any other crate builds.

export type ReadFile = (path: string) => string | undefined;

function crateOf(path: string, crates: readonly Member[]): Member | undefined {
  const owners = crates.filter((crate) => crate.folder !== "" && path.startsWith(`${crate.folder}/`));
  return owners.sort((a, b) => b.folder.length - a.folder.length)[0];
}

function parentFiles(folder: string, parent: string[]): string[] {
  if (parent.length === 0) return [`${folder}/src/lib.rs`, `${folder}/src/main.rs`];
  const module = `${folder}/src/${parent.join("/")}`;
  return [`${module}.rs`, `${module}/mod.rs`];
}

function onlyInTests(line: string | undefined): boolean {
  if (line === "#[cfg(test)]") return true;
  const all = /^#\[cfg\(all\((.*)\)\)\]$/.exec(line ?? "")?.[1];
  if (all === undefined) return false;
  let depth = 0;
  const top = [...all].map((char) => {
    depth += char === "(" ? 1 : char === ")" ? -1 : 0;
    return char === "," && depth === 0 ? "\0" : char;
  });
  return top.join("").split("\0").some((part) => part.trim() === "test");
}

function declarations(text: string, module: string): boolean[] {
  const declared = new RegExp(`^(pub(\\([^)]*\\))? )?mod ${module};$`);
  const lines = text.split("\n").map((line) => line.trim());
  return lines.flatMap((line, at) => (declared.test(line) ? [onlyInTests(lines[at - 1])] : []));
}

const TEST_MODULE = /^(\w+_)?tests$/;

/** A parent file in `changed` is not trusted: `read` gives it as it is now. */
export function testOwner(path: string, crates: readonly Member[], read: ReadFile, changed: ReadonlySet<string>): string | undefined {
  const crate = crateOf(path, crates);
  if (crate === undefined || !path.endsWith(".rs")) return undefined;
  const parts = path.slice(crate.folder.length + 1).split("/");
  if (parts[0] === "tests" && parts.length > 1) return crate.name;
  if (parts[0] !== "src") return undefined;
  const module = (part: string, index: number) => (index === parts.length - 1 ? part.replace(/\.rs$/, "") : part);
  const at = parts.findIndex((part, index) => index > 0 && TEST_MODULE.test(module(part, index)));
  const name = parts[at];
  if (at === -1 || name === undefined) return undefined;
  const found = parentFiles(crate.folder, parts.slice(1, at)).flatMap((file) => {
    if (changed.has(file)) return [false];
    const text = read(file);
    return text === undefined ? [] : declarations(text, module(name, at));
  });
  return found.length > 0 && found.every(Boolean) ? crate.name : undefined;
}

/** No doctest: a doctest reads no `cfg(test)` code and no file of `tests/`. `pinning` is the test of the shell. */
export function crateGates(gates: readonly Gate[], crates: readonly string[]): Gate[] {
  const flags = crates.flatMap((crate) => ["-p", crate]);
  return gates.flatMap((gate): Gate[] => {
    if (gate.name === "clippy") return [{ ...gate, argv: ["cargo", "clippy", ...flags, "--all-targets", "--", "-D", "warnings"] }];
    if (gate.name === "rust-test") return [{ ...gate, argv: ["bun", "quality/bin/with-postgres.ts", "cargo", "nextest", "run", ...flags, "--no-tests=pass"] }];
    if (gate.name === "doctest") return [];
    if (gate.name === "pinning" && !crates.includes("anima-app")) return [];
    return [gate];
  });
}
