import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const script = join(import.meta.dir, "..", "testflight", "swift-package.sh");
const tag = "refs/tags/1.0.9";
const revision = "207e4bfe5d36fdd0cc08f28d1849ff94ac7e24b8";

test.skipIf(process.platform === "win32")("SwiftPM's Git URLs resolve the original release with network transports disabled", () => {
  const temporary = mkdtempSync(join(tmpdir(), "anima-swift-test-"));
  try {
    const offline = { ...process.env, GIT_ALLOW_PROTOCOL: "file" };
    const prepared = Bun.spawnSync(["sh", script, temporary], { env: offline });
    expect(prepared.exitCode, prepared.stderr.toString()).toBe(0);
    const mirror = Object.fromEntries(
      prepared.stdout.toString().trimEnd().split("\n").map((line) => {
        const separator = line.indexOf("=");
        return [line.slice(0, separator), line.slice(separator + 1)];
      }),
    );
    for (const suffix of ["", ".git"]) {
      const output = Bun.spawnSync(["git", "ls-remote", `https://github.com/Brendonovich/swift-rs${suffix}`, tag], {
        env: { ...offline, ...mirror },
      });
      expect(output.exitCode, output.stderr.toString()).toBe(0);
      expect(output.stdout.toString().trim()).toBe(`${revision}\t${tag}`);
    }
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
