import { afterEach, beforeEach } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

export const SCRIPT = join(import.meta.dir, "..", "bin", "anima-install");
export const VERSION = "0.9.40";
export const TARGET = process.platform === "darwin" ? "darwin-universal" : "linux-x86_64";
export const CLAIM = "C".repeat(43);
export const INVITE = "anima-invite.1.SU5WSVRFLUNPREUtMTIzNA.fd41-hub";
export const ENROL = `anima_enrol_${"E".repeat(22)}`;
export const CODE = `anima-install.1.${CLAIM}.${INVITE}`;

let room: string;
export let home: string;
let release: string;
export let log: string;

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

export const DAEMON = standIn("anima-daemon", `[ "$1" = enrol ] && printf 'stdin %s\\n' "$(cat)" >> "$STAND_IN_LOG"
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

export function write(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  chmodSync(path, 0o755);
}

export function installerFixtures(): void {
  beforeEach(() => {
    room = mkdtempSync(join(tmpdir(), "anima-install-"));
    home = join(room, "home");
    release = join(room, "release");
    log = join(room, "log");
    mkdirSync(home);
    write(join(release, TARGET, VERSION, "anima-link"), LINK);
    write(join(release, TARGET, VERSION, "anima-daemon"), DAEMON);
    write(join(room, "bin", "curl"), CURL);
    write(join(room, "bin", "loginctl"), standIn("loginctl", "exit 1"));
    writeFileSync(log, "");
  });

  afterEach(() => {
    rmSync(room, { recursive: true, force: true });
  });

}

interface Run { code: number; out: string; err: string }

interface Change { link?: string; daemon?: string; target?: string; env?: Record<string, string>; options?: string[]; answer?: string }

export function install(code: string, change: Change = {}): Run {
  const args = ["/bin/sh", SCRIPT, "--version", VERSION, "--target", change.target ?? TARGET,
    "--link-sha256", change.link ?? sha256(LINK), "--daemon-sha256", change.daemon ?? sha256(DAEMON), "--from", release, ...change.options ?? []];
  if (change.answer !== undefined) {
    return run(["/usr/bin/python3", join(import.meta.dir, "anima-install-terminal.py"), ...args],
      JSON.stringify({ code, answer: change.answer }), change.env);
  }
  return run(args, `${code}\n`, change.env);
}

export function run(args: string[], stdin: string, env: Record<string, string> = {}): Run {
  const result = Bun.spawnSync(args, {
    stdin: new TextEncoder().encode(stdin),
    env: { PATH: `${join(room, "bin")}:/usr/bin:/bin:/usr/sbin:/sbin`, HOME: home, STAND_IN_LOG: log, ...env },
    stdout: "pipe",
    stderr: "pipe",
  });
  return { code: result.exitCode, out: result.stdout.toString(), err: result.stderr.toString() };
}

export const remove = (): Run => run(["/bin/sh", SCRIPT, "--remove"], "");
export const linkHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-link") : join(home, ".local/share/anima-link");
export const daemonHome = (): string => process.platform === "darwin" ? join(home, "Library/Application Support/anima-daemon") : join(home, ".config/anima-daemon");
export const daemonCopy = (): string => join(home, ".local/bin/anima-daemon");
export const lines = (): string[] => readFileSync(log, "utf8").split("\n").filter((line) => line !== "");
export const commands = (): string[] => lines().filter((line) => !line.startsWith("stdin "));

export function appUnit(state: string): void {
  const file = join(home, "app-unit.plist");
  writeFileSync(file, `<array>\n        <string>/Applications/Anima.app/link</string>\n        <string>-state</string>\n        <string>${state}</string>\n</array>\n`);
  writeFileSync(join(home, "unit-file"), file);
}


export function lingerFixture(system = "Linux", state = "no", outcome = "yes", uid = "1001"): void {
  write(join(release, system === "Linux" ? "linux-x86_64" : "darwin-universal", VERSION, "anima-link"), LINK);
  write(join(release, system === "Linux" ? "linux-x86_64" : "darwin-universal", VERSION, "anima-daemon"), DAEMON);
  write(join(room, "bin", "uname"), `#!/bin/sh
[ "$1" = -s ] && echo ${system} || echo x86_64
`);
  write(join(room, "bin", "id"), `#!/bin/sh
echo ${uid}
`);
  writeFileSync(join(home, "linger"), state);
  write(join(room, "bin", "loginctl"), standIn("loginctl", `
case "$*" in
  "--no-ask-password show-user ${uid} -p Linger --value")
    if [ "${state}" = failure ]; then echo yes; exit 1; fi
    cat "$HOME/linger" ;;
  "--no-ask-password enable-linger ${uid}")
    [ "${outcome}" != fail ] || { echo "${ENROL}" >&2; exit 1; }
    echo '${outcome}' > "$HOME/linger" ;;
  *) exit 2 ;;
esac`));
}
