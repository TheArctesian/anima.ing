import type { Gate } from "./gates.ts";

export interface Member {
  folder: string;
  name: string;
  uses?: string[];
}

export interface Layout {
  crates: Member[];
  packages: Member[];
}

export type Touched = "all" | string[];

export type Choice =
  | { tier: "nothing"; reasons: string[] }
  | { tier: "documents"; reasons: string[] }
  | { tier: "code"; rust: Touched; go: boolean; bun: Touched; reasons: string[] }
  | { tier: "full"; reasons: string[] };

const RUST_ROOT = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "clippy.toml", "rustfmt.toml", ".config/", ".cargo/"];
/** `design/` holds the tokens that the client reads. */
const BUN_ROOT = ["package.json", "bun.lock", "bun.lockb", "bunfig.toml", "tsconfig.json", "tsconfig.base.json", "eslint.config.js", "eslint.config.ts", "design/"];
/** `contracts/` holds the GraphQL schema that the command line and the client compile against. */
const CONTRACTS = "contracts/";
const GO_MODULE = "link/";
/** The tests of the deploy package read the workflow files. */
const WORKFLOWS = ".github/";
const WORKFLOW_TESTS = "@anima/deploy";

/** Deploy tests that read the Rust workspace; `release-decision.test.ts` does too but takes a minute, so only GitHub runs it. */
const WORKSPACE_MAPS = [
  "deploy/test/release-applications.test.ts",
  "deploy/test/release-gates.test.ts",
  "deploy/test/cli-release.test.ts",
  "deploy/test/compose.test.ts",
  "deploy/test/compose-hub.test.ts",
];

export function workspaceMaps(rust: Touched, deployTested: boolean): Gate[] {
  if ((rust !== "all" && rust.length === 0) || deployTested) return [];
  return [{ name: "deploy-maps", group: "bun", argv: ["bun", "test", "--timeout", "30000", ...WORKSPACE_MAPS] }];
}

const testsDeploy = (bun: Touched) => bun === "all" || bun.includes(WORKFLOW_TESTS);

const NEUTRAL = [".gitattributes", "LICENSE"];

export function isDocument(path: string): boolean {
  return path.startsWith("docs/") || (!path.includes("/") && path.endsWith(".md")) || NEUTRAL.includes(path);
}

function under(path: string, prefix: string): boolean {
  return prefix.endsWith("/") ? path.startsWith(prefix) : path === prefix;
}

/** The longest folder wins, so `apps/desktop/src-tauri` is the crate. */
function ownerOf(path: string, members: { member: Member; rust: boolean }[]) {
  return members
    .filter(({ member }) => path.startsWith(`${member.folder}/`))
    .sort((one, other) => other.member.folder.length - one.member.folder.length)[0];
}

function add(touched: Touched, name: string): Touched {
  if (touched === "all" || touched.includes(name)) return touched;
  return [...touched, name];
}

/** A path that no rule knows gives the full list: the fast tier never guesses that a file is safe. */
export function choose(paths: string[], layout: Layout): Choice {
  if (paths.length === 0) return { tier: "nothing", reasons: ["no file is different from the base"] };
  if (paths.every(isDocument)) return { tier: "documents", reasons: [`${paths.length} changed file(s), each a document`] };
  const members = [
    ...layout.crates.map((member) => ({ member, rust: true })),
    ...layout.packages.map((member) => ({ member, rust: false })),
  ];
  let rust: Touched = [];
  let bun: Touched = [];
  let go = false;
  const reasons: string[] = [];
  for (const path of paths.filter((each) => !isDocument(each))) {
    const owner = ownerOf(path, members);
    if (path.startsWith(GO_MODULE)) {
      go = true;
    } else if (path.startsWith(CONTRACTS)) {
      [rust, bun] = ["all", "all"];
      reasons.push(`${path}: a contract, so each Rust and bun package`);
    } else if (RUST_ROOT.some((root) => under(path, root))) {
      rust = "all";
      reasons.push(`${path}: each Rust package`);
    } else if (BUN_ROOT.some((root) => under(path, root))) {
      bun = "all";
      reasons.push(`${path}: each bun package`);
    } else if (path.startsWith(WORKFLOWS)) {
      bun = add(bun, WORKFLOW_TESTS);
    } else if (owner?.rust) {
      rust = add(rust, owner.member.name);
    } else if (owner !== undefined) {
      bun = add(bun, owner.member.name);
    } else {
      return { tier: "full", reasons: [`${path}: no rule of the fast tier knows this path`] };
    }
  }
  return { tier: "code", rust, go, bun, reasons };
}

function packageFlags(names: string[]): string[] {
  return names.flatMap((name) => ["-p", name]);
}

function rustGates(full: Gate[], touched: Touched): Gate[] {
  if (touched === "all") return full.filter((gate) => gate.group === "rust");
  if (touched.length === 0) return [];
  const named = (name: string) => full.filter((gate) => gate.name === name);
  const flags = packageFlags(touched);
  return [
    ...named("fmt"),
    { name: "clippy", group: "rust", argv: ["cargo", "clippy", ...flags, "--all-targets", "--", "-D", "warnings"] },
    {
      name: "rust-test",
      group: "rust",
      // A package with no test does not fail the tier: the full list still runs each test.
      argv: ["bun", "quality/bin/with-postgres.ts", "cargo", "nextest", "run", ...flags, "--no-tests=pass"],
      requires: named("rust-test")[0]?.requires,
    },
    ...(touched.includes("anima-app") ? named("pinning") : []),
  ];
}

function bunGates(full: Gate[], touched: Touched): Gate[] {
  if (touched === "all") return full.filter((gate) => gate.group === "bun");
  if (touched.length === 0) return [];
  const filters = touched.flatMap((name) => ["--filter", name]);
  const script = (name: string): Gate => ({ name, group: "bun", argv: ["bun", "run", ...filters, name] });
  return [
    ...full.filter((gate) => gate.name === "install"),
    script("lint"),
    script("check"),
    script("test"),
    ...(touched.includes("@anima/client") ? full.filter((gate) => gate.name === "build") : []),
  ];
}

/** Paths whose tests a Linux machine of GitHub does not run or prove (R-254 item 3). */
const MAC_ONLY = [
  "apps/desktop/src-tauri/",
  "crates/daemon/src/keys/",
  "crates/daemon/src/token_store",
  "crates/daemon/src/units/",
  "link/statedir/",
  "deploy/test/check-mach-o",
  "deploy/test/trial-bundle",
];

export function macOnly(paths: string[]): string[] {
  return paths.filter((path) => MAC_ONLY.some((prefix) => path.startsWith(prefix)));
}

export function isBusy(load: number, limit: number): boolean {
  return load > limit;
}

const TEST_FILE = /\.test\.[cm]?[jt]sx?$/;

function narrowBunTests(touched: string[], paths: string[], packages: Member[]): Gate[] {
  const files: string[] = [];
  const whole: string[] = [];
  for (const name of touched) {
    const folder = packages.find((member) => member.name === name)?.folder;
    const inPackage = folder === undefined ? [] : paths.filter((path) => path.startsWith(`${folder}/`));
    const tests = inPackage.filter((path) => TEST_FILE.test(path));
    if (tests.length > 0) files.push(...tests);
    else if (inPackage.length > 0) whole.push(name);
  }
  const filters = [...files, ...(paths.some((path) => path.startsWith(WORKFLOWS)) ? ["workflow"] : [])];
  return [
    ...(filters.length > 0 ? [{ name: "test", group: "bun" as const, argv: ["bun", "test", ...filters] }] : []),
    ...(whole.length > 0 ? [{ name: "test", group: "bun" as const, argv: ["bun", "run", ...whole.flatMap((name) => ["--filter", name]), "test"] }] : []),
  ];
}

const BROAD = ["rust-test", "doctest", "pinning", "test", "build"];

/** R-268. */
function busyGates(choice: Extract<Choice, { tier: "code" }>, full: Gate[], busy: Busy): Gate[] {
  const light = (gates: Gate[]) => gates.filter((gate) => !BROAD.includes(gate.name));
  const rust = choice.rust === "all" ? light(rustGates(full, "all")) : rustGates(full, choice.rust);
  const bun = choice.bun === "all" ? light(bunGates(full, "all")) : [...light(bunGates(full, choice.bun)), ...narrowBunTests(choice.bun, busy.paths, busy.packages)];
  const deployWhole = choice.bun !== "all" && choice.bun.includes(WORKFLOW_TESTS) && !busy.paths.some((path) => path.startsWith("deploy/") && TEST_FILE.test(path));
  return [
    ...rust,
    ...(choice.go ? full.filter((gate) => gate.group === "go") : []),
    ...bun,
    ...workspaceMaps(choice.rust, deployWhole),
    ...full.filter((gate) => gate.group === "quality"),
  ];
}

export interface Busy {
  paths: string[];
  packages: Member[];
}

/** Each gate comes from `full`, so that a gate has one definition. */
export function fastGates(choice: Choice, full: Gate[], busy?: Busy): Gate[] {
  switch (choice.tier) {
    case "nothing":
      return [];
    case "documents":
      return full.filter((gate) => gate.name === "documents");
    case "full":
      return full;
    case "code":
      if (busy !== undefined && macOnly(busy.paths).length === 0) return busyGates(choice, full, busy);
      return [
        ...rustGates(full, choice.rust),
        ...(choice.go ? full.filter((gate) => gate.group === "go") : []),
        ...bunGates(full, choice.bun),
        ...workspaceMaps(choice.rust, testsDeploy(choice.bun)),
        ...full.filter((gate) => gate.group === "quality"),
      ];
  }
}

export function describeChoice(choice: Choice): string {
  const part = (label: string, touched: Touched) =>
    touched === "all" ? `${label}: each package` : touched.length > 0 ? `${label}: ${touched.join(", ")}` : undefined;
  const parts =
    choice.tier === "code"
      ? [part("Rust", choice.rust), choice.go ? "Go: the link" : undefined, part("bun", choice.bun)].filter((each) => each !== undefined)
      : [];
  return [`fast tier: ${choice.tier}`, ...parts, ...choice.reasons].join("; ");
}
