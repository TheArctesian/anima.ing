import { afterAll, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const room = mkdtempSync(join(tmpdir(), "anima-setup-"));
afterAll(() => rmSync(room, { recursive: true, force: true }));
const setup = readFileSync(join(import.meta.dir, "../bin/anima-setup"), "utf8");

function fixture() {
  const home = mkdtempSync(join(room, "user-"));
  const bundle = join(home, "bundle");
  mkdirSync(bundle);
  writeFileSync(join(bundle, "anima-setup"), setup);
  for (const name of ["anima", "anima-link", "anima-daemon"]) {
    writeFileSync(join(bundle, name), `#!/bin/sh
[ -z "\${ANIMA_TOKEN:-}" ] || exit 81
[ -z "\${ANIMA_HUB:-}" ] || exit 82
printf '%s\\n' "$*" >> "$HOME/calls"
case "$*" in *"auth status"*) exit 1 ;; esac
exit 0
`);
    chmodSync(join(bundle, name), 0o755);
  }
  const path = join(home, "tools");
  mkdirSync(path);
  writeFileSync(join(path, "id"), '#!/bin/sh\ncase "$1" in -u) echo 1001 ;; -un) echo test-user ;; esac\n');
  chmodSync(join(path, "id"), 0o755);
  return { home, bundle, path };
}

function run(args: string[]) {
  const { home, bundle, path } = fixture();
  const result = Bun.spawnSync(["sh", join(bundle, "anima-setup"), ...args], {
    env: { ...process.env, HOME: home, XDG_CONFIG_HOME: join(home, "config"), PATH: `${path}:${process.env.PATH}`, ANIMA_TOKEN: "synthetic-token-must-not-be-used", ANIMA_HUB: "https://wrong.invalid" },
    stdout: "pipe", stderr: "pipe",
  });
  return { home, result, calls: existsSync(join(home, "calls")) ? readFileSync(join(home, "calls"), "utf8") : "" };
}

test("first setup installs before approval and enrols without inherited credentials", () => {
  const { home, result, calls } = run(["--computer", "Build VPS"]);
  expect(result.exitCode).toBe(0);
  for (const name of ["anima", "anima-link", "anima-daemon"]) expect(existsSync(join(home, ".local/bin", name))).toBe(true);
  expect(calls.split("\n").filter(Boolean).map((line) => line.includes("auth status") ? "status" : line.includes("auth login") ? "login" : "enrol")).toEqual(["status", "login", "enrol"]);
  expect(calls).toContain("auth login --hosted --computer Build VPS");
  expect(calls).toContain("daemon install --computer Build VPS");
  expect(result.stderr.toString()).not.toContain("synthetic-token");
});

test("CLI-only setup installs programs without approval or device enrolment", () => {
  const { home, result, calls } = run(["--cli-only"]);
  expect(result.exitCode).toBe(0);
  expect(existsSync(join(home, ".local/bin/anima"))).toBe(true);
  expect(calls).toBe("");
});
