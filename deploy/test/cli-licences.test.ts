// deploy/release/licences.txt and each place that gives out `anima`: the archives of `anima`
// alone (deploy/bin/cli-archive and the Windows zip of release.yml), the Homebrew formula and the
// desktop bundles. Apache-2.0 section 4 asks that each copy carry the licence and the NOTICE text
// of crates/shortening. Nothing here builds a release.
import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..");
const ROOM = mkdtempSync(join(tmpdir(), "cli-licences-"));
afterAll(() => {
  rmSync(ROOM, { recursive: true, force: true });
});

/** The lines of licences.txt: the file of the repository and its name in a copy. */
const LICENCES = readFileSync(join(ROOT, "deploy", "release", "licences.txt"), "utf8")
  .split("\n")
  .filter((line) => line.trim() !== "" && !line.startsWith("#"))
  .map((line) => {
    const [source = "", name = ""] = line.trim().split(/\s+/);
    return { source, name };
  });

function run(program: string[], env: Record<string, string> = {}): { exitCode: number; printed: string } {
  const done = Bun.spawnSync(program, { cwd: ROOM, stdout: "pipe", stderr: "pipe", env: { ...process.env, ...env } });
  return { exitCode: done.exitCode, printed: done.stdout.toString() };
}

const GNU_TAR = run(["tar", "--version"]).printed.includes("GNU tar");

describe("the licence files of a copy of anima", () => {
  test("the list names the licence of Anima.ing and the licence and NOTICE of crates/shortening, each file present", () => {
    expect(LICENCES.map(({ source }) => source)).toEqual(["LICENSE", "crates/shortening/NOTICE", "crates/shortening/LICENSE-APACHE"]);
    for (const { source } of LICENCES) expect(existsSync(join(ROOT, source)), source).toBe(true);
    expect(new Set(LICENCES.map(({ name }) => name)).size).toBe(LICENCES.length);
  });

  test.skipIf(!GNU_TAR)("cli-archive packs anima with each licence file, and the same inputs give the same bytes", () => {
    const folder = join(ROOM, "programs");
    mkdirSync(folder, { recursive: true });
    writeFileSync(join(folder, "anima"), "#!/bin/sh\necho anima 0.8.15\n");
    const pack = (archive: string) => run(["/bin/sh", join(ROOT, "deploy", "bin", "cli-archive"), folder, join(ROOM, archive), "1700000000"]);
    expect(pack("one.tar.gz").exitCode).toBe(0);
    expect(pack("two.tar.gz").exitCode).toBe(0);
    expect(readFileSync(join(ROOM, "one.tar.gz")).equals(readFileSync(join(ROOM, "two.tar.gz")))).toBe(true);
    const listed = run(["tar", "-tvzf", join(ROOM, "one.tar.gz")]).printed.trim().split("\n");
    expect(listed.map((line) => line.split(/\s+/).at(-1)).sort()).toEqual(["anima", ...LICENCES.map(({ name }) => name)].sort());
    expect(listed.find((line) => line.endsWith(" anima"))?.startsWith("-rwxr-xr-x")).toBe(true);
    const out = join(ROOM, "out");
    mkdirSync(out, { recursive: true });
    expect(run(["tar", "-xzf", join(ROOM, "one.tar.gz"), "-C", out]).exitCode).toBe(0);
    for (const { source, name } of LICENCES) expect(readFileSync(join(out, name), "utf8")).toBe(readFileSync(join(ROOT, source), "utf8"));
  });

  test.skipIf(process.platform === "win32")("cli-archive refuses a folder with no anima", () => {
    const empty = join(ROOM, "empty");
    mkdirSync(empty, { recursive: true });
    expect(run(["/bin/sh", join(ROOT, "deploy", "bin", "cli-archive"), empty, join(ROOM, "none.tar.gz"), "1700000000"]).exitCode).toBe(1);
    expect(existsSync(join(ROOM, "none.tar.gz"))).toBe(false);
  });

  test.skipIf(process.platform === "win32")("the Homebrew formula names both licences and installs each licence file", () => {
    const text = run(["/bin/sh", join(ROOT, "deploy", "bin", "cli-formula"), "0.8.15", "a".repeat(64), "b".repeat(64)]).printed;
    expect(text).toContain('  license all_of: ["AGPL-3.0-or-later", "Apache-2.0"]\n');
    expect(text).toContain(`    prefix.install ${LICENCES.map(({ name }) => `"${name}"`).join(", ")}\n`);
  });

  test("the release packs each archive of anima with cli-archive, and the Windows zips of the release and the trial with the list", () => {
    const release = readFileSync(join(ROOT, ".github", "workflows", "release.yml"), "utf8");
    expect(release).toContain('sh deploy/bin/cli-archive "$mac" out/anima-darwin-universal.tar.gz "$TIME"');
    expect(release).toContain('sh deploy/bin/cli-archive "$linux" out/anima-linux-x86_64.tar.gz "$TIME"');
    expect(release).not.toMatch(/-C "\$1" anima \|/);
    expect(release).toContain("foreach ($line in Get-Content deploy/release/licences.txt)");
    expect(release).toContain("Compress-Archive -Path cli-stage/* -DestinationPath out-cli/anima-windows-x86_64.zip");
    const trial = readFileSync(join(ROOT, ".github", "workflows", "windows-trial.yml"), "utf8");
    expect(trial).toContain("foreach ($line in Get-Content deploy/release/licences.txt)");
    expect(trial).toContain("Compress-Archive -Path cli-stage/* -DestinationPath out-cli/anima-windows-x86_64.zip");
  });

  test("the Windows bundle of the Builds view (builder/carried.rs) names each licence file under licences/", () => {
    const carried = readFileSync(join(ROOT, "apps", "desktop", "src-tauri", "src", "builder", "carried.rs"), "utf8");
    const list = /const LICENCES: \[\(&str, &str\); \d+\] = \[([\s\S]*?)\];/.exec(carried)?.[1] ?? "";
    const pairs = [...list.matchAll(/\(\s*"([^"]+)",\s*"([^"]+)",?\s*\)/g)].map(([, source, target]) => [source, target]);
    expect(pairs).toEqual(LICENCES.map(({ source, name }) => [`../../../${source}`, `licences/${name}`]));
  });

  // Windows: in the file that only the bundle step reads (windows-bundle.ps1), beside the tree of the database.
  test.each([
    ["macos", "tauri.macos.conf.json", {}],
    ["linux", "tauri.linux.conf.json", {}],
    ["windows", "tauri.windows-carried.json", { "database/": "database/" }],
  ])("the desktop bundle for %s carries each licence file under licences/ (%s)", (_platform, file, others) => {
    const config = JSON.parse(readFileSync(join(ROOT, "apps", "desktop", "src-tauri", file), "utf8")) as { bundle?: { resources?: Record<string, string> } };
    const licences = Object.fromEntries(LICENCES.map(({ source, name }) => [`../../../${source}`, `licences/${name}`]));
    expect(config.bundle?.resources).toEqual({ ...others, ...licences });
  });
});
