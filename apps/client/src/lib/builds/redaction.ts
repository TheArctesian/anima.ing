/** The first of two redaction passes before a log line reaches the hub, so it errs towards removing too much (BLD-049, LOG-007). */

const REDACTED = '[redacted]';
/** A known value shorter than this is not looked for: replacing `a` everywhere would ruin the log and hide nothing. */
const SHORTEST_SECRET = 8;

/** A URL whose query carries a signature or a credential: a presigned upload URL (BLD-060). */
const SIGNED_URL = /\bhttps?:\/\/\S*[?&](?:X-Amz-Signature|X-Amz-Credential|X-Amz-Security-Token|Signature|sig|token)=\S*/gi;

/** Base64 of minisign secret-key lines (`EdScB2`, `Ed\0\0B2`) and of whole minisign files (`untrusted comment:`), as Tauri keeps them. */
const MINISIGN = /(?:RWRTY0Iy|RWQAAEIy|dW50cnVzdGVkIGNvbW1lbnQ6)[A-Za-z0-9+/=]*/g;

const KEY_ASSIGNMENT = /\b(TAURI_SIGNING_PRIVATE_KEY(?:_PASSWORD)?|TAURI_PRIVATE_KEY(?:_PASSWORD)?)\s*[=:]\s*\S+/g;

const BLOCK_START = /-----BEGIN [A-Z0-9 ]*PRIVATE KEY( BLOCK)?-----/;
const BLOCK_END = /-----END [A-Z0-9 ]*PRIVATE KEY( BLOCK)?-----/;

/** What the shell puts after a line it cut at 4096 bytes (BLD-048). */
const CUT = ' [line cut]';

/** `text` with its end removed when that end is the start of `secret`: a cut can split a secret in two. */
function withoutSecretStart(text: string, secret: string): string {
  for (let length = Math.min(secret.length - 1, text.length); length > 0; length -= 1) {
    if (text.endsWith(secret.slice(0, length))) return `${text.slice(0, -length)}${REDACTED}`;
  }
  return text;
}

function withoutKnown(line: string, secrets: readonly string[]): string {
  const cut = line.endsWith(CUT);
  let text = cut ? line.slice(0, -CUT.length) : line;
  for (const secret of secrets) {
    if (secret.length < SHORTEST_SECRET) continue;
    text = text.replaceAll(secret, REDACTED);
    if (cut) text = withoutSecretStart(text, secret);
  }
  return cut ? `${text}${CUT}` : text;
}

export function cleanLine(line: string, secrets: readonly string[]): string {
  return withoutKnown(line, secrets)
    .replace(SIGNED_URL, REDACTED)
    .replace(MINISIGN, REDACTED)
    .replace(KEY_ASSIGNMENT, `$1=${REDACTED}`);
}

export class LineCleaner {
  #inBlock = false;
  readonly #secrets: () => readonly string[];

  /** `secrets` is read for each line, so a token stored later is still removed. */
  constructor(secrets: () => readonly string[]) {
    this.#secrets = secrets;
  }

  clean(line: string): string {
    if (this.#inBlock || BLOCK_START.test(line)) {
      this.#inBlock = !BLOCK_END.test(line);
      return REDACTED;
    }
    return cleanLine(line, this.#secrets());
  }
}
