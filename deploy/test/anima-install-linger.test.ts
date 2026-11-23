import { describe, expect, test } from "bun:test";
import { installerFixtures, CLAIM, CODE, commands, ENROL, install, INVITE, lingerFixture } from "./anima-install.fixture";

installerFixtures();

const enable = "loginctl --no-ask-password enable-linger 1001";
const linux = { target: "linux-x86_64" };

describe.skipIf(process.platform === "win32")("Linux enrollment lingering consent (HM-T33)", () => {
  test.each(["", "\nyes", "\nyes\nyes"])("piped install code with trailing %j never grants consent", (trailing) => {
    lingerFixture();
    const result = install(CODE + trailing, linux);
    expect(result.code, result.err).toBe(0);
    expect(commands()).not.toContain(enable);
    expect(result.err).toContain("--enable-linger");
  });

  test.each(["no\n", "\n", "y\n", "YES\n", "\u0004"])("terminal refusal %j preserves enrollment without enabling", (answer) => {
    lingerFixture();
    const result = install(CODE, { ...linux, answer });
    expect(result.code, result.err).toBe(0);
    expect(result.err).toContain("all user units");
    expect(result.err).toContain("not enabled");
    expect(commands()).not.toContain(enable);
    expect(commands()).toContain("anima-daemon install");
  });

  test.each([false, true])("explicit opt-in configures and checks current UID (terminal=%s)", (terminal) => {
    lingerFixture();
    const result = install(CODE, { ...linux, ...terminal ? { answer: "yes\n" } : { options: ["--enable-linger"] } });
    expect(result.code, result.err).toBe(0);
    expect(commands().filter((line) => line.startsWith("loginctl"))).toEqual([
      "loginctl --no-ask-password show-user 1001 -p Linger --value", enable,
      "loginctl --no-ask-password show-user 1001 -p Linger --value",
    ]);
    expect(result.err).toContain("lingering is enabled");
    for (const secret of [CLAIM, INVITE, ENROL]) {
      expect(commands().join("\n") + result.out + result.err).not.toContain(secret);
    }
  });

});

describe.skipIf(process.platform === "win32")("Linux lingering outcomes (HM-T33)", () => {
  test("an interrupted prompt never enables lingering", () => {
    lingerFixture();
    const result = install(CODE, { ...linux, answer: "\u0003" });
    expect(result.code).not.toBe(0);
    expect(commands()).not.toContain(enable);
  });

  test("root is refused before enrollment or lingering", () => {
    lingerFixture("Linux", "no", "yes", "0");
    const result = install(CODE, { ...linux, options: ["--enable-linger"] });
    expect(result.code).toBe(1);
    expect(commands()).toEqual([]);
    expect(result.err).toContain("not as root");
  });

  test("an enabled user needs no prompt or change", () => {
    lingerFixture("Linux", "yes");
    const result = install(CODE, linux);
    expect(result.code, result.err).toBe(0);
    expect(result.err).toContain("already enabled");
    expect(commands()).not.toContain(enable);
  });

  test.each(["fail", "no", "unknown"])("failed enable or unverified state (%s) never reports configured", (outcome) => {
    lingerFixture("Linux", "no", outcome);
    const result = install(CODE, { ...linux, options: ["--enable-linger"] });
    expect(result.code).toBe(1);
    expect(commands()).toContain(enable);
    expect(result.err).toContain("enrollment remains installed");
    expect(result.err).toContain("administrator");
    expect(result.err).not.toContain("lingering is enabled");
    expect(result.err).not.toContain(ENROL);
  });

  test.each(["unknown", "failure"])("unreadable status (%s) never changes the system", (state) => {
    lingerFixture("Linux", state);
    const result = install(CODE, { ...linux, options: ["--enable-linger"] });
    expect(result.code).toBe(1);
    expect(commands()).not.toContain(enable);
    expect(result.err).toContain("could not read");
  });

  test("macOS keeps its LaunchAgent path without a lingering call", () => {
    lingerFixture("Darwin");
    const result = install(CODE, { target: "darwin-universal", options: ["--enable-linger"] });
    expect(result.code, result.err).toBe(0);
    expect(commands().some((line) => line.startsWith("loginctl"))).toBe(false);
    expect(result.err).toContain("Linux only");
  });
});
