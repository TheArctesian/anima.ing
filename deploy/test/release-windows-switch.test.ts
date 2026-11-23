// Unix deployment/release scripts run in the Linux/macOS gates. Windows uses its native bundle/runtime.
// The switch of Windows in .github/workflows/release.yml (ANIMA_PUBLIC_WINDOWS): with it not
// `on`, a release builds, signs and publishes macOS and Linux, and no step needs a file of
// Windows. The steps here run the scripts of the workflow on files of a temporary folder; none
// reads a secret or the network.
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

interface Step {
  name?: string;
  uses?: string;
  if?: string;
  run?: string;
  env?: Record<string, string>;
  with?: Record<string, string>;
}
interface Job {
  needs?: string | string[];
  if?: string;
  steps?: Step[];
}

const RAW = readFileSync(join(import.meta.dir, "..", "..", ".github", "workflows", "release.yml"), "utf8");
const jobs = (Bun.YAML.parse(RAW) as { jobs: Record<string, Job> }).jobs;
const RELEASE_WINDOWS = "${{ needs.commit.outputs.release_windows }}";
const VERSION = "0.0.1";

function steps(name: string): Step[] {
  const found = jobs[name];
  if (found === undefined) throw new Error(`the workflow has no job ${name}`);
  return found.steps ?? [];
}

function step(job: string, part: string): Step {
  const found = steps(job).find((one) => (one.name ?? "").includes(part));
  if (found?.run === undefined) throw new Error(`the job ${job} has no step ${part}`);
  return found;
}

/** The jobs that wait for the job `windows`. */
const AFTER_WINDOWS = Object.entries(jobs).flatMap(([name, found]) => ([found.needs ?? []].flat().includes("windows") ? [name] : []));

let folder = "";
beforeEach(() => {
  folder = mkdtempSync(join(tmpdir(), "release-windows-"));
  // The runner has GNU sha256sum; macOS has none, and its shasum reads the same lines.
  mkdirSync(join(folder, "bin"));
  writeFileSync(join(folder, "bin", "sha256sum"), '#!/bin/sh\nexec shasum -a 256 "$@"\n');
  chmodSync(join(folder, "bin", "sha256sum"), 0o755);
});
afterEach(() => {
  rmSync(folder, { recursive: true, force: true });
});

/** Writes `files` below the folder and gives the SHA-256 of each. */
function place(files: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(files).map(([path, text]) => {
      mkdirSync(dirname(join(folder, path)), { recursive: true });
      writeFileSync(join(folder, path), text);
      return [path, createHash("sha256").update(text).digest("hex")];
    }),
  );
}

const GUARD = 'if [ "$WINDOWS" = true ]; then';

/** The lines of `script` that run only while `$WINDOWS` is true: one line `if ...; then ...; fi`, or the body of an `if ...; then` up to its `fi`. */
function guardedLines(script: string): Set<string> {
  const guarded = new Set<string>();
  // The indent of the open `if`, or undefined outside one.
  let open: string | undefined;
  for (const line of script.split("\n")) {
    const trimmed = line.trim();
    const indent = line.slice(0, line.length - line.trimStart().length);
    if (open !== undefined && trimmed === "fi" && indent === open) open = undefined;
    else if (open !== undefined) guarded.add(line);
    else if (trimmed === GUARD) open = indent;
    else if (trimmed.startsWith(`${GUARD} `) && trimmed.endsWith("; fi")) guarded.add(line);
  }
  return guarded;
}

function run(script: string, env: Record<string, string>): number {
  const path = `${join(folder, "bin")}:${process.env["PATH"] ?? "/usr/bin:/bin"}`;
  return Bun.spawnSync(["sh", "-c", script], { cwd: folder, env: { PATH: path, VERSION, ...env }, stdout: "pipe", stderr: "pipe" }).exitCode;
}

describe("the switch of Windows (ANIMA_PUBLIC_WINDOWS)", () => {
  test("only the switch `on` builds Windows, in a release and in a trial", () => {
    const read = steps("commit").find((one) => one.env?.["WINDOWS_SWITCH"] !== undefined);
    expect(read?.env?.["WINDOWS_SWITCH"]).toBe("${{ vars.ANIMA_PUBLIC_WINDOWS }}");
    expect(read?.run).toContain('if [ "$release" = true ] && [ "$WINDOWS_SWITCH" = on ]; then release_windows=true; fi');
    expect(read?.run).toMatch(/if \[ "\$WINDOWS_SWITCH" = on \]; then\n\s+windows=\$\(published windows-x86_64\)/);
    expect(jobs["windows"]?.if).toBe("needs.commit.outputs.release_windows == 'true'");
  });

  test("the check of guarded lines refuses a line after the `fi` and an inverted guard", () => {
    const guarded = guardedLines(`a\n${GUARD}\n  cp windows/x .\nfi\ncp windows/y .\n[ "$WINDOWS" = true ] || cp windows/z .`);
    expect([...guarded]).toEqual(["  cp windows/x ."]);
  });

  test("each job after it reads its files only in a run that built them", () => {
    expect(AFTER_WINDOWS.sort()).toEqual(["cli", "publish", "sign", "trial"]);
    for (const name of AFTER_WINDOWS) {
      for (const one of steps(name).filter((each) => /^windows/.test(each.with?.["name"] ?? ""))) {
        expect(one.if).toBe("needs.commit.outputs.release_windows == 'true'");
      }
    }
    for (const name of [...AFTER_WINDOWS, "cli-publish"]) {
      for (const one of steps(name)) {
        const guarded = guardedLines(one.run ?? "");
        const windowsLines = (one.run ?? "").split("\n").filter((line) => /windows-x86_64|x64-setup|windows-cli|windows\/|WINDOWS_SHA256/.test(line));
        for (const line of windowsLines) expect(guarded.has(line), `${name}: ${line.trim()}`).toBe(true);
        if (windowsLines.length > 0) expect(one.env?.["WINDOWS"], `${name}: ${one.name ?? ""}`).toBe(RELEASE_WINDOWS);
      }
    }
  });

});

// These run sh; the Windows gates have no sh (as the other deploy tests that run a script).
describe.skipIf(process.platform === "win32")("the steps of a run with and without Windows", () => {
  test("the sign job compares the Windows installer only when the run built it", () => {
    const compare = step("sign", "Compare each file");
    const digests = place({
      "tool/anima-hub": "tool",
      "macos/Anima.app.tar.gz": "mac",
      [`linux/Anima_${VERSION}_amd64.AppImage`]: "linux",
      "daemon/darwin-universal/anima-daemon": "daemon mac",
      "daemon/linux-x86_64/anima-daemon": "daemon linux",
      "daemon/darwin-universal/anima-link": "link mac",
      "daemon/linux-x86_64/anima-link": "link linux",
    });
    const env = {
      TOOL_SHA256: digests["tool/anima-hub"] ?? "",
      MACOS_SHA256: digests["macos/Anima.app.tar.gz"] ?? "",
      LINUX_SHA256: digests[`linux/Anima_${VERSION}_amd64.AppImage`] ?? "",
      WINDOWS_SHA256: "",
      DAEMON_MACOS_SHA256: digests["daemon/darwin-universal/anima-daemon"] ?? "",
      DAEMON_LINUX_SHA256: digests["daemon/linux-x86_64/anima-daemon"] ?? "",
      LINK_MACOS_SHA256: digests["daemon/darwin-universal/anima-link"] ?? "",
      LINK_LINUX_SHA256: digests["daemon/linux-x86_64/anima-link"] ?? "",
    };
    expect(run(compare.run ?? "", { ...env, WINDOWS: "false" })).toBe(0);
    expect(run(compare.run ?? "", { ...env, WINDOWS: "true" })).not.toBe(0);
    const installer = place({ [`windows/Anima_${VERSION}_x64-setup.exe`]: "installer" });
    expect(run(compare.run ?? "", { ...env, WINDOWS: "true", WINDOWS_SHA256: installer[`windows/Anima_${VERSION}_x64-setup.exe`] ?? "" })).toBe(0);
  });

  test("the publish job reads back the Windows feed only when it wrote it", () => {
    const readBack = step("publish", "Read the feeds");
    const log = join(folder, "read");
    writeFileSync(join(folder, "bin", "curl"), `#!/bin/sh\necho "$@" >> "${log}"\necho '{"version":"${VERSION}"}'\n`);
    chmodSync(join(folder, "bin", "curl"), 0o755);
    const env = { ANIMA_RELEASES_PUBLIC_BASE: "https://releases.example" };
    expect(run(readBack.run ?? "", { ...env, WINDOWS: "false" })).toBe(0);
    expect(readFileSync(log, "utf8")).not.toContain("windows-x86_64");
    expect(readFileSync(log, "utf8")).toContain("linux-x86_64/latest.json");
    rmSync(log);
    expect(run(readBack.run ?? "", { ...env, WINDOWS: "true" })).toBe(0);
    expect(readFileSync(log, "utf8")).toContain("windows-x86_64/latest.json");
  });
});
