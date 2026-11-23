import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const SCRIPT = join(import.meta.dir, "..", "bin", "anima-install");
const VERSION = "0.9.40";
const TARGET = process.platform === "darwin" ? "darwin-universal" : "linux-x86_64";
const CLAIM = "C".repeat(43);
const INVITE = "anima-invite.1.SU5WSVRFLUNPREUtMTIzNA.fd41-hub";
const ENROL = `anima_enrol_${"E".repeat(22)}`;
const CODE = `anima-install.1.${CLAIM}.${INVITE}`;

let room: string;
let home: string;
let release: string;
let log: string;

function standIn(name: string, body: string): string {
  return `#!/bin/sh
printf '%s %s\\n' '${name}' "$*" >> "$STAND_IN_LOG"
${body}
`;
}

const LINK = standIn("anima-link", `unit="$HOME/units/anima-link.service"
case $1 in
  join) printf 'stdin %s\\n' "$(cat)" >> "$STAND_IN_LOG" ;;
  unit) case $2 in
          show) if [ -f "$HOME/unit-json" ]; then cat "$HOME/unit-json"
                elif [ -f "$HOME/unit-file" ]; then printf '{"installed":true,"file":"%s"}\\n' "$(cat "$HOME/unit-file")"
                else printf '{"installed":false,"file":"%s"}\\n' "$unit"; fi ;;
          install) mkdir -p "$HOME/units"; printf 'ExecStart="%s" "device" "-state" "%s" "-listen" "%s"\\n' "$0" "$4" "$6" > "$unit"; echo "$unit" > "$HOME/unit-file" ;;
          remove) rm -f "$(cat "$HOME/unit-file")" "$HOME/unit-file" ;;
        esac ;;
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
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  chmodSync(path, 0o755);
}

beforeEach(() => {
  room = mkdtempSync(join(tmpdir(), "anima-install-"));
  home = join(room, "home");
  release = join(room, "release");
  log = join(room, "log");
  mkdirSync(home);
  write(join(release, TARGET, VERSION, "anima-link"), LINK);
  write(join(release, TARGET, VERSION, "anima-daemon"), DAEMON);
  write(join(room, "bin", "curl"), CURL);
  writeFileSync(log, "");
});

afterEach(() => {
  rmSync(room, { recursive: true, force: true });
});

interface Run { code: number; out: string; err: string }

interface Change { link?: string; daemon?: string; target?: string; env?: Record<string, string> }

function install(code: string, change: Change = {}): Run {
  const args = ["/bin/sh", SCRIPT, "--version", VERSION, "--target", change.target ?? TARGET,
    "--link-sha256", change.link ?? sha256(LINK), "--daemon-sha256", change.daemon ?? sha256(DAEMON), "--from", release];
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

const remove = (): Run => run(["/bin/sh", SCRIPT, "--remove"], "");
const linkHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-link") : join(home, ".local/share/anima-link");
const daemonHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-daemon") : join(home, ".config/anima-daemon");
const daemonCopy = (): string => join(home, ".local/bin/anima-daemon");
const lines = (): string[] => readFileSync(log, "utf8").split("\n").filter((line) => line !== "");
const commands = (): string[] => lines().filter((line) => !line.startsWith("stdin "));

function appUnit(state: string): void {
  const file = join(home, "app-unit.plist");
  writeFileSync(file, `<array>\n        <string>/Applications/Anima.app/link</string>\n        <string>-state</string>\n        <string>${state}</string>\n</array>\n`);
  writeFileSync(join(home, "unit-file"), file);
}

describe.skipIf(process.platform === "win32")("the one-line installation of a computer with no application (HM-T33)", () => {
  test("it places both programs, joins with the invite, installs the link unit with the daemon socket, claims the code and enrols the daemon", () => {
    const result = install(CODE);
    expect(result.code, result.err).toBe(0);
    expect(existsSync(join(linkHome(), "program/anima-link"))).toBe(true);
    expect(existsSync(daemonCopy())).toBe(true);
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

  test("the claim goes to the hub through the socket of the daemon, never through the port of the link", () => {
    expect(install(CODE).code).toBe(0);
    const sent = commands().filter((line) => line.startsWith("curl") && line.includes("graphql"));
    expect(sent).toHaveLength(1);
    expect(sent[0]).toContain(`--unix-socket ${join(daemonHome(), "link.sock")} http://link/graphql`);
    expect(sent[0]).not.toContain("127.0.0.1");
  });

  test("no secret of the run is an argument of a program or in the output", () => {
    const result = install(CODE);
    expect(result.code, result.err).toBe(0);
    for (const secret of [CLAIM, INVITE, ENROL]) {
      expect(commands().join("\n")).not.toContain(secret);
      expect(result.out + result.err).not.toContain(secret);
    }
  });

  test.each([["anima-link", { link: "0".repeat(64) }], ["anima-daemon", { daemon: "0".repeat(64) }]])(
    "%s whose SHA-256 is not the one of the line stops the run before anything is installed",
    (program, change) => {
      const result = install(CODE, change);
      expect(result.code).toBe(1);
      expect(result.err).toContain(`${program} ${VERSION} is not the file of the line: its SHA-256 differs`);
      expect(existsSync(join(linkHome(), "program/anima-link"))).toBe(false);
      expect(existsSync(daemonCopy())).toBe(false);
      expect(commands().filter((line) => !line.includes("unit show"))).toEqual([]);
    },
  );
});

describe.skipIf(process.platform === "win32")("a second run of the one-line installation (HM-T33)", () => {
  test("a second run with a claim only does not join again and enrols the daemon again", () => {
    expect(install(CODE).code).toBe(0);
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
    expect(install(CODE).code).toBe(0);
    const again = install(CODE);
    expect(again.code).toBe(1);
    expect(again.err).toContain("on the link already");
  });

  test("a claim that the hub refuses stops the run before the daemon enrols", () => {
    const result = install(CODE, { env: { CLAIM_ANSWER: "refused" } });
    expect(result.code).toBe(1);
    expect(result.err).toContain("refused the claim");
    expect(commands().some((line) => line.startsWith("anima-daemon"))).toBe(false);
  });

  test("a daemon that this script did not enrol is never enrolled over", () => {
    write(join(daemonHome(), "daemon.json"), "{}");
    const result = install(CODE);
    expect(result.code).toBe(1);
    expect(result.err).toContain("a daemon of Anima is enrolled for this user already");
    expect(commands()).toEqual([]);
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
    expect(existsSync(linkHome())).toBe(false);
    expect(existsSync(daemonCopy())).toBe(false);
  });

  test("a line for a different computer is refused", () => {
    const other = TARGET === "darwin-universal" ? "linux-x86_64" : "darwin-universal";
    const result = install(CODE, { target: other });
    expect(result.code).toBe(1);
    expect(result.err).toContain(`the line is for ${other}`);
  });

  test("an option with no value says so", () => {
    const result = run(["/bin/sh", SCRIPT, "--version"], "");
    expect(result.code).toBe(1);
    expect(result.err).toContain("--version needs a value");
  });

  test.each([
    ["names a different state folder", (): void => { appUnit("/Users/someone/Library/Application Support/Anima/link"); }],
    ["names a state folder that only starts like this one", (): void => { appUnit(`${join(linkHome(), "state")}-old`); }],
  ])("the unit of the application is never taken when it %s: the run stops", (_name, unit) => {
    unit();
    const result = install(CODE);
    expect(result.code).toBe(1);
    expect(result.err).toContain("the application keeps the link");
    expect(existsSync(daemonCopy())).toBe(false);
  });

  test("an installed unit whose file is not a plain path in the JSON of `unit show` stops the run", () => {
    writeFileSync(join(home, "unit-json"), `{"installed":true,"file":"${join(home, "a\\u0026b.plist")}"}\n`);
    const result = install(CODE);
    expect(result.code).toBe(1);
    expect(result.err).toContain("its file is not a plain path");
    expect(existsSync(daemonCopy())).toBe(false);
  });
});

describe.skipIf(process.platform === "win32")("the removal of the one-line installation (HM-T33)", () => {
  test("--remove takes out both units, both programs and both folders, and says that the hub still lists the computer", () => {
    expect(install(CODE).code).toBe(0);
    writeFileSync(log, "");
    const result = remove();
    expect(result.code, result.err).toBe(0);
    expect(commands()).toContain("anima-daemon uninstall");
    expect(commands()).toContain("anima-link unit remove");
    for (const gone of [linkHome(), daemonHome(), daemonCopy()]) expect(existsSync(gone)).toBe(false);
    expect(result.err).toContain("The hub still lists this computer");
  });

  test("--remove leaves a daemon that this script did not install", () => {
    write(join(daemonHome(), "daemon.json"), "{}");
    write(daemonCopy(), DAEMON);
    const result = remove();
    expect(result.code, result.err).toBe(0);
    expect(commands()).not.toContain("anima-daemon uninstall");
    expect(existsSync(join(daemonHome(), "daemon.json"))).toBe(true);
    expect(existsSync(daemonCopy())).toBe(true);
  });

  test("--remove stops, and removes nothing, when the application keeps the link", () => {
    expect(install(CODE).code).toBe(0);
    appUnit("/Users/someone/Library/Application Support/Anima/link");
    writeFileSync(log, "");
    const result = remove();
    expect(result.code).toBe(1);
    expect(result.err).toContain("the application keeps the link");
    expect(commands().filter((line) => !line.includes("unit show"))).toEqual([]);
    for (const kept of [linkHome(), daemonHome(), daemonCopy()]) expect(existsSync(kept)).toBe(true);
  });
});
