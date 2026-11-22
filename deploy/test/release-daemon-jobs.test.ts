// Rules for the jobs of the daemon alone in .github/workflows/release.yml (task U2; ROL-002 to
// ROL-005) and of the one-line install (HM-T33, R-361): where the files come from, who signs
// their record, and that nothing of it is public before the desktop application of the same run. Nothing here runs the workflow or reads a
// secret; no run has proved these jobs.
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import { ownCondition } from "./release-waits.ts";

interface Step {
  name?: string;
  uses?: string;
  run?: string;
  env?: Record<string, string>;
  with?: Record<string, string>;
}
interface Job {
  needs?: string | string[];
  if?: string;
  environment?: string;
  outputs?: Record<string, string>;
  steps: Step[];
}

const ROOT = join(import.meta.dir, "..", "..");
const RAW = readFileSync(join(ROOT, ".github", "workflows", "release.yml"), "utf8");
const jobs = (Bun.YAML.parse(RAW) as { jobs: Record<string, Job> }).jobs;

function job(name: string): Job {
  const found = jobs[name];
  if (found === undefined) throw new Error(`the workflow has no job ${name}`);
  return found;
}
const needs = (name: string): string[] => [job(name).needs ?? []].flat();
const text = (name: string): string => job(name).steps.map((step) => step.run ?? "").join("\n");
const PUBLIC = "github.event_name == 'push' && github.ref == 'refs/heads/main' && vars.ANIMA_PUBLIC_RELEASES == 'on'";

describe("the daemon alone in a release", () => {
  test("ROL-002 R-361 its files are the programs that the build job put in each application; the job builds nothing and has no secret", () => {
    expect(needs("daemon").sort()).toEqual(["commit", "linux", "macos"]);
    expect(job("daemon").if).toBe("needs.commit.outputs.release == 'true'");
    expect(job("daemon").environment).toBeUndefined();
    const run = text("daemon");
    expect(run).toContain("for program in anima-daemon anima-link; do");
    expect(run).toContain('tar -xzf macos/Anima.app.tar.gz -C work/mac "Anima.ing.app/Contents/MacOS/$program"');
    expect(run).toContain('--appimage-extract "usr/bin/$program"');
    expect(run).toContain("Mach-O universal binary with 2 architectures");
    expect(run).toContain('if ldd "work/linux/$program/squashfs-root/usr/bin/$program" | grep \'not found\'; then');
    expect(run).not.toMatch(/(^|[\s;&|(])(cargo|bun|go|git) /);
    expect(Object.keys(job("daemon").outputs ?? {}).sort()).toEqual(["install_sha256", "link_linux_sha256", "link_macos_sha256", "linux_sha256", "macos_sha256"]);
  });

  test("ROL-004 the job that holds the updater key signs each record, after it compared the two files with their digests", () => {
    const sign = text("sign");
    expect(sign).toContain('echo "$DAEMON_MACOS_SHA256  daemon/darwin-universal/anima-daemon"');
    expect(sign).toContain('echo "$DAEMON_LINUX_SHA256  daemon/linux-x86_64/anima-daemon"');
    expect(sign).toContain('echo "$LINK_MACOS_SHA256  daemon/darwin-universal/anima-link"');
    expect(sign).toContain('echo "$LINK_LINUX_SHA256  daemon/linux-x86_64/anima-link"');
    expect(sign.indexOf("sha256sum --check --strict")).toBeLessThan(sign.indexOf("release record linux-x86_64"));
    expect(sign).toContain('tool/anima-hub release record linux-x86_64 "$VERSION" daemon/linux-x86_64');
    expect(sign).toContain('tool/anima-hub release record darwin-universal "$VERSION" daemon/darwin-universal');
    const step = job("sign").steps.find((one) => (one.run ?? "").includes("release record"));
    expect(step?.env).toMatchObject({
      ANIMA_RELEASE_COMMIT: "${{ needs.commit.outputs.sha }}",
      ANIMA_RELEASE_REPLACES_FROM: "${{ needs.commit.outputs.replaces_from }}",
      SOURCE_DATE_EPOCH: "${{ needs.commit.outputs.time }}",
    });
  });

  test("ROL-041 the lowest version that a release can replace is one line of the repository, read by the first job", () => {
    const value = readFileSync(join(ROOT, "deploy", "release", "replaces-from"), "utf8")
      .split("\n")
      .filter((line) => line !== "" && !line.startsWith("#"));
    expect(value).toEqual(["0.0.0"]);
    expect(text("commit")).toContain("grep -v '^#' deploy/release/replaces-from");
    expect(text("commit")).toContain('echo "replaces_from=$replaces_from"');
  });

  test("ROL-005 nothing of it is public before the desktop application of the run, and only with the switch on", () => {
    expect(needs("daemon-publish").sort()).toEqual(["commit", "daemon", "publish", "sign"]);
    expect(ownCondition(job("daemon-publish").if, needs("daemon-publish"))).toBe(PUBLIC);
    expect(job("daemon-publish").environment).toBe("release-publish");
    for (const name of ["sign", "publish", "trial"]) expect(needs(name)).not.toContain("daemon-publish");
  });

  test("ROL-005 R-361 for each target the programs come first, then the record, then its signature; the install script comes last", () => {
    const run = text("daemon-publish");
    const order = ["anima-daemon\" \"$target/$VERSION/anima-daemon\"", "anima-link\" \"$target/$VERSION/anima-link\"", "release-record.json\" \"$target/$VERSION/release-record.json\"", "release-record.json.sig\" \"$target/$VERSION/release-record.json.sig\"", "put install/anima-install.sh \"install/$VERSION/anima-install.sh\" \"text/plain; charset=utf-8\""].map((key) => run.indexOf(key));
    expect(order.every((at) => at > 0)).toBe(true);
    expect([...order].sort((a, b) => a - b)).toEqual(order);
    expect(run).toContain("for target in linux-x86_64 darwin-universal; do");
    expect(run).toContain('echo "::error::The bucket holds $2 with other bytes."');
    expect(run).toContain('echo "$INSTALL_SHA256  install/anima-install.sh"');
    expect(run.indexOf("sha256sum --check --strict")).toBeLessThan(run.indexOf("put-object"));
    expect(run).not.toMatch(/delete-object|copy-object|latest\.json/);
  });
});

describe("the one-line install in a release", () => {
  test("R-361 the install script is the file of the release commit, outside the folders that a record lists", () => {
    const checkout = job("daemon").steps.find((step) => (step.uses ?? "").startsWith("actions/checkout@"));
    expect(checkout?.with).toMatchObject({ ref: "${{ needs.commit.outputs.sha }}", path: "source", "sparse-checkout": "deploy/bin/anima-install", "persist-credentials": false });
    const run = text("daemon");
    expect(run).toContain("cp source/deploy/bin/anima-install daemon/install/anima-install.sh");
    expect(text("sign")).not.toContain("daemon/install");
  });
});
