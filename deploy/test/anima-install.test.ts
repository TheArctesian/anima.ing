// Unix deployment/release scripts run in the Linux/macOS gates. Windows uses its native bundle/runtime.
// deploy/bin/anima-install: the one-line installation of the link and of the daemon on a computer
// with no application (hosted-mode.md HM-T33, R-240 c). It runs against a fake release folder,
// with stand-ins for anima-link, anima-daemon and curl that write down what they were given: no
// network, no unit and no hub.
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SCRIPT = join(import.meta.dir, "..", "bin", "anima-install");
const VERSION = "0.9.40";
const TARGET = process.platform === "darwin" ? "darwin-universal" : "linux-x86_64";
const CLAIM = "C".repeat(43);
const INVITE = "anima-invite.1.SU5WSVRFLUNPREUtMTIzNA.fd41-hub";
const ENROL = `anima_enrol_${"E".repeat(22)}`;

let room: string;
let home: string;
let release: string;
let log: string;

/** A stand-in program: it writes its name and arguments, then its standard input, to the log. */
function standIn(name: string, body: string): string {
  return `#!/bin/sh
printf '%s %s\\n' '${name}' "$*" >> "$STAND_IN_LOG"
${body}
`;
}

const LINK = standIn("anima-link", `case $1 in
  join) printf 'stdin %s\\n' "$(cat)" >> "$STAND_IN_LOG" ;;
  unit) [ "$2" = show ] && { if [ -f "$HOME/unit-file" ]; then printf '{"installed":true,"file":"%s"}\\n' "$(cat "$HOME/unit-file")"; else echo '{"installed":false,"file":""}'; fi; }
        [ "$2" = install ] && { printf 'state %s\\n' "$4" > "$HOME/unit"; echo "$HOME/unit" > "$HOME/unit-file"; }
        [ "$2" = remove ] && rm -f "$HOME/unit" "$HOME/unit-file" ;;
esac
exit 0`);

const DAEMON = standIn("anima-daemon", `[ "$1" = enrol ] && printf 'stdin %s\\n' "$(cat)" >> "$STAND_IN_LOG"
exit 0`);

/** curl: a link that answers /health and the claim; anything else is a closed port. */
const CURL = standIn("curl", `case "$*" in
  *graphql*) printf 'stdin %s\\n' "$(cat)" >> "$STAND_IN_LOG"; printf '{"data":{"claimComputerEnrolment":{"code":"%s"}}}' "\${CLAIM_ANSWER:-${ENROL}}" ;;
  */health*) exit 0 ;;
  *) exit 7 ;;
esac`);

function sha256(text: string): string {
  return createHash("sha256").update(text).digest("hex");
}

function write(path: string, text: string): void {
  writeFileSync(path, text);
  chmodSync(path, 0o755);
}

beforeEach(() => {
  room = mkdtempSync(join(tmpdir(), "anima-install-"));
  home = join(room, "home");
  release = join(room, "release");
  log = join(room, "log");
  mkdirSync(home);
  mkdirSync(join(release, TARGET, VERSION), { recursive: true });
  mkdirSync(join(room, "bin"));
  write(join(release, TARGET, VERSION, "anima-link"), LINK);
  write(join(release, TARGET, VERSION, "anima-daemon"), DAEMON);
  write(join(room, "bin", "curl"), CURL);
  writeFileSync(log, "");
});

afterEach(() => {
  rmSync(room, { recursive: true, force: true });
});

interface Run { code: number; out: string; err: string }

function install(code: string, change: { link?: string; target?: string; env?: Record<string, string> } = {}): Run {
  const args = ["/bin/sh", SCRIPT, "--version", VERSION, "--target", change.target ?? TARGET,
    "--link-sha256", change.link ?? sha256(LINK), "--daemon-sha256", sha256(DAEMON), "--from", release];
  return run(args, `${code}\n`, change.env);
}

function run(args: string[], stdin: string, env: Record<string, string> = {}): Run {
  const result = Bun.spawnSync(args, {
    stdin: new TextEncoder().encode(stdin),
    env: { PATH: `${join(room, "bin")}:/usr/bin:/bin:/usr/sbin:/sbin`, HOME: home, STAND_IN_LOG: log, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  return { code: result.exitCode, out: result.stdout.toString(), err: result.stderr.toString() };
}

const linkHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-link") : join(home, ".local/share/anima-link");
const daemonHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-daemon") : join(home, ".config/anima-daemon");
const lines = (): string[] => readFileSync(log, "utf8").split("\n").filter((line) => line !== "");
/** The lines that hold the arguments of a program, not its standard input. */
const commands = (): string[] => lines().filter((line) => !line.startsWith("stdin "));

describe.skipIf(process.platform === "win32")("the one-line installation of a computer with no application (HM-T33)", () => {
  test("it places both programs, joins with the invite, installs the link unit with the daemon socket, claims the code and enrols the daemon", () => {
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`);
    expect(result.code, result.err).toBe(0);
    expect(existsSync(join(linkHome(), "program/anima-link"))).toBe(true);
    expect(existsSync(join(home, ".local/bin/anima-daemon"))).toBe(true);
    const state = join(linkHome(), "state");
    const socket = join(daemonHome(), "link.sock");
    const order = commands().filter((line) => !line.includes("unit show") && !line.startsWith("curl"));
    expect(order[0]).toBe(`anima-link join -state ${state}`);
    expect(order[1]).toMatch(new RegExp(`^anima-link unit install -state ${state} -listen 127\\.0\\.0\\.1:\\d+ -daemon-socket ${socket}$`));
    expect(order[2]).toBe(`anima-daemon enrol --hub unix:${socket} -`);
    expect(order[3]).toBe("anima-daemon install");
    expect(lines()).toContain(`stdin ${INVITE}`);
    expect(lines()).toContain(`stdin ${ENROL}`);
    const claim = lines().find((line) => line.startsWith("stdin {"));
    expect(JSON.parse(claim?.slice("stdin ".length) ?? "{}").variables).toEqual({ claim: CLAIM, os: process.platform === "darwin" ? "MACOS" : "LINUX" });
  });

  test("no secret of the run is an argument of a program or in the output", () => {
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`);
    expect(result.code, result.err).toBe(0);
    for (const secret of [CLAIM, INVITE, ENROL]) {
      expect(commands().join("\n")).not.toContain(secret);
      expect(result.out + result.err).not.toContain(secret);
    }
  });

  test("a program whose SHA-256 is not the one of the line stops the run before anything is installed", () => {
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`, { link: "0".repeat(64) });
    expect(result.code).toBe(1);
    expect(result.err).toContain("its SHA-256 differs");
    expect(existsSync(join(linkHome(), "program/anima-link"))).toBe(false);
    expect(existsSync(join(home, ".local/bin/anima-daemon"))).toBe(false);
    expect(commands()).toEqual([]);
  });

});

describe.skipIf(process.platform === "win32")("a second run of the one-line installation (HM-T33)", () => {
  test("a second run with a claim only does not join again and enrols the daemon again", () => {
    expect(install(`anima-install.1.${CLAIM}.${INVITE}`).code).toBe(0);
    writeFileSync(log, "");
    const result = install(`anima-install.1.${CLAIM}`);
    expect(result.code, result.err).toBe(0);
    expect(commands().some((line) => line.includes(" join "))).toBe(false);
    expect(commands()).toContain("anima-daemon install");
    const ports = readFileSync(join(linkHome(), "port"), "utf8");
    expect(commands().find((line) => line.includes("unit install"))).toContain(`127.0.0.1:${ports.trim()}`);
  });

  test("a new invite on a computer that joined already, and a claim only on one that has not, are refused", () => {
    expect(install(`anima-install.1.${CLAIM}`).err).toContain("Make the install code with a new name");
    expect(install(`anima-install.1.${CLAIM}.${INVITE}`).code).toBe(0);
    const again = install(`anima-install.1.${CLAIM}.${INVITE}`);
    expect(again.code).toBe(1);
    expect(again.err).toContain("on the link already");
  });

  test("a claim that the hub refuses stops the run before the daemon enrols", () => {
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`, { env: { CLAIM_ANSWER: "refused" } });
    expect(result.code).toBe(1);
    expect(result.err).toContain("refused the claim");
    expect(commands().some((line) => line.startsWith("anima-daemon"))).toBe(false);
  });

});

describe.skipIf(process.platform === "win32")("the refusals of the one-line installation (HM-T33)", () => {
  test.each([
    ["no install code", ""],
    ["a code of a different form", "anima_enrol_AAAAAAAAAAAAAAAAAAAAAA"],
    ["a claim of the wrong length", "anima-install.1.short"],
  ])("%s is refused before anything is downloaded", (_name, code) => {
    const result = install(code);
    expect(result.code).toBe(1);
    expect(commands()).toEqual([]);
  });

  test("a line for a different computer is refused", () => {
    const other = TARGET === "darwin-universal" ? "linux-x86_64" : "darwin-universal";
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`, { target: other });
    expect(result.code).toBe(1);
    expect(result.err).toContain(`the line is for ${other}`);
  });

  test("the unit of the application is never taken: the run stops", () => {
    writeFileSync(join(home, "app-unit"), "ProgramArguments: /Applications/Anima.app/link\n");
    writeFileSync(join(home, "unit-file"), join(home, "app-unit"));
    const result = install(`anima-install.1.${CLAIM}.${INVITE}`);
    expect(result.code).toBe(1);
    expect(result.err).toContain("the application keeps the link");
  });

  test("--remove takes out both units, both programs and both folders, and says that the hub still lists the computer", () => {
    expect(install(`anima-install.1.${CLAIM}.${INVITE}`).code).toBe(0);
    writeFileSync(log, "");
    const result = run(["/bin/sh", SCRIPT, "--remove"], "");
    expect(result.code, result.err).toBe(0);
    expect(commands()).toContain("anima-daemon uninstall");
    expect(commands()).toContain("anima-link unit remove");
    for (const gone of [linkHome(), daemonHome(), join(home, ".local/bin/anima-daemon")]) expect(existsSync(gone)).toBe(false);
    expect(result.err).toContain("The hub still lists this computer");
  });
});
