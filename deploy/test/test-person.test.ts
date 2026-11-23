// R-315: the feature `test-person` of the daemon takes the process that started the runner for
// the person. Only the tests of the daemon may have it. A release build refuses it
// (`compile_error!` without debug assertions, crates/daemon/src/threads/runner/peer.rs), so these
// tests pin the two ways it could still reach a shipped program: a release profile with debug
// assertions, and a build that turns the feature on.
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "..", "..");

const tracked = new TextDecoder()
  .decode(Bun.spawnSync(["git", "ls-files"], { cwd: ROOT }).stdout)
  .split("\n")
  .filter((path) => path.length > 0);

type Table = Record<string, unknown>;

const manifests = tracked
  .filter((path) => path === "Cargo.toml" || path.endsWith("/Cargo.toml"))
  .map((path) => ({ path, toml: Bun.TOML.parse(readFileSync(join(ROOT, path), "utf8")) as Table }));

/** Each profile that is a release profile: `release`, or one that inherits from one. */
function releaseProfiles(profiles: Table): [string, Table][] {
  const release = new Set(["release"]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const [name, profile] of Object.entries(profiles)) {
      const inherits = (profile as Table).inherits;
      if (!release.has(name) && typeof inherits === "string" && release.has(inherits)) {
        release.add(name);
        grew = true;
      }
    }
  }
  return Object.entries(profiles)
    .filter(([name]) => release.has(name))
    .map(([name, profile]) => [name, profile as Table]);
}

/** The files that build or start a build: workflows, scripts, images, the release builder. */
const builds = tracked.filter(
  (path) =>
    /\.(ya?ml|sh)$/.test(path) ||
    /(^|\/)Dockerfile[^/]*$/.test(path) ||
    path.endsWith("package.json") ||
    (path.startsWith("quality/") && path.endsWith(".ts") && !path.endsWith(".test.ts")) ||
    path.startsWith("apps/desktop/src-tauri/src/builder/"),
);

describe("R-315 the feature test-person never reaches a shipped program", () => {
  test("R-315 no release profile of the workspace turns debug assertions on", () => {
    expect(manifests.length).toBeGreaterThan(0);
    for (const { path, toml } of manifests) {
      const profiles = (toml.profile ?? {}) as Table;
      for (const [name, profile] of releaseProfiles(profiles)) {
        const packages = Object.values((profile.package ?? {}) as Table) as Table[];
        for (const settings of [profile, ...packages]) {
          expect(`${path} [profile.${name}] debug-assertions=${String(settings["debug-assertions"])}`).not.toContain(
            "debug-assertions=true",
          );
        }
      }
    }
  });

  test("R-315 no workflow, gate, image or build script turns on each feature or test-person", () => {
    expect(builds.length).toBeGreaterThan(0);
    for (const path of builds) {
      const text = readFileSync(join(ROOT, path), "utf8");
      expect(`${path}: ${text.includes("--all-features") ? "--all-features" : ""}`).toBe(`${path}: `);
      expect(`${path}: ${text.includes("test-person") ? "test-person" : ""}`).toBe(`${path}: `);
    }
  });

  test("R-315 only the daemon's own tests turn the feature on", () => {
    const users = manifests.filter(({ path }) => readFileSync(join(ROOT, path), "utf8").includes("test-person"));
    expect(users.map(({ path }) => path)).toEqual(["crates/daemon/Cargo.toml"]);
    const daemon = users[0]?.toml ?? {};
    const self = ((daemon["dev-dependencies"] ?? {}) as Table)["anima-daemon"] as Table | undefined;
    expect(self?.features).toEqual(["test-person"]);
    for (const section of ["dependencies", "build-dependencies"]) {
      expect(JSON.stringify(daemon[section] ?? {})).not.toContain("test-person");
    }
    const features = (daemon.features ?? {}) as Record<string, string[]>;
    expect(features["test-person"]).toEqual([]);
    for (const [name, enables] of Object.entries(features)) {
      if (name !== "test-person") expect(enables).not.toContain("test-person");
    }
    expect(features.default ?? []).not.toContain("test-person");
  });
});
