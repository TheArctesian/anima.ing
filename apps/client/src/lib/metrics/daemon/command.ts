import type { DaemonRelease, DaemonTarget } from './release';

/** `link`: a daemon cannot use the local port of the client's link (MET-019; K8 Section 1, R-172; HM-D16). */
export type CommandRefusal = 'code' | 'origin' | 'link';

export type InstallCommand =
  | { readonly kind: 'download'; readonly text: string }
  /** Development: the daemon is already on the computer; the operator pastes the code at its prompt. */
  | { readonly kind: 'enrol-only'; readonly text: string; readonly note: string };

export type CommandResult = { readonly ok: true; readonly command: InstallCommand } | { readonly ok: false; readonly refusal: CommandRefusal };

export interface CommandInput {
  readonly release: DaemonRelease | null;
  readonly target: DaemonTarget | null;
  /** The enrolment code; it is read only to put it between single quotes of a download command. */
  readonly code: string;
  readonly origin: string;
}

/** K8 Section 3: `anima_enrol_` and the unpadded base64url of 16 bytes, which is 22 characters. */
const CODE = /^anima_enrol_[A-Za-z0-9_-]{22}$/;
/** An https origin as the daemon takes it (MET-020): the daemon refuses any port but 443. */
const ORIGIN = /^https:\/\/[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9])?(?::443)?$/;

const LINK_ORIGIN = /^http:\/\/127\.0\.0\.1:\d{1,5}$/;

const DOWNLOAD_ROOT = 'https://releases.anima.ing/daemon';
const DAEMON = '~/.local/bin/anima-daemon';

/** POSIX single quotes; a quote inside the text is closed, escaped and reopened, so no text leaves them. */
function quote(text: string): string {
  return `'${text.replaceAll("'", `'\\''`)}'`;
}

const PASTE = 'Run this where the daemon is installed and paste the code at its prompt. The code works once.';
const NO_RELEASE_NOTE = `No download is published for this build. ${PASTE}`;
const NO_TARGET_NOTE = `Choose the computer above to get the full install command. ${PASTE}`;
const NO_BINARY_NOTE = `No download is published for this computer. ${PASTE}`;

/** MET-019, D-10 b. */
export function installCommand({ release, target, code, origin }: CommandInput): CommandResult {
  if (LINK_ORIGIN.test(origin)) return { ok: false, refusal: 'link' };
  if (!ORIGIN.test(origin)) return { ok: false, refusal: 'origin' };
  const sha256 = release !== null && target !== null ? release.sha256[target] : undefined;
  if (release === null || target === null || sha256 === undefined) {
    const note = release === null ? NO_RELEASE_NOTE : target === null ? NO_TARGET_NOTE : NO_BINARY_NOTE;
    return { ok: true, command: { kind: 'enrol-only', text: `anima-daemon enrol --hub ${quote(origin)} -`, note } };
  }
  if (!CODE.test(code)) return { ok: false, refusal: 'code' };
  const verify = target.startsWith('macos') ? 'shasum -a 256 -c -' : 'sha256sum -c -';
  const url = `${DOWNLOAD_ROOT}/v${release.version}/anima-daemon-${target}`;
  const steps = [
    't=$(mktemp)',
    `trap 'rm -f "$t"' EXIT`,
    `trap 'exit 130' INT HUP TERM`,
    `curl -fsSL --proto '=https' --proto-redir '=https' ${quote(url)} -o "$t"`,
    `printf '%s  %s\\n' ${quote(sha256)} "$t" | ${verify}`,
    'mkdir -p ~/.local/bin',
    `install -m 0755 "$t" ${DAEMON}`,
    `printf '%s\\n' ${quote(code)} | ${DAEMON} enrol --hub ${quote(origin)} -`,
    `${DAEMON} install`,
  ];
  return { ok: true, command: { kind: 'download', text: `(${steps.join(' && ')})` } };
}

export function refusalMessage(refusal: CommandRefusal): string {
  if (refusal === 'link') return 'No install command: this application reaches the hub through its link, and a daemon cannot use that path yet.';
  return refusal === 'origin'
    ? 'No install command: the hub address is not a plain https origin.'
    : 'No install command: the daemon does not accept the form of the hub\'s code.';
}
