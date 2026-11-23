// Never-log shapes (LOG-007, SEC-009); each pattern bounds its repeats, so matching time is linear in the line.
// Known misses: lower-case compound field names, `+` as a space in form bodies, and values inside base64.

export type ShapeKind =
  | "api-key"
  | "daemon-token"
  | "enrolment-code"
  | "browser-cookie"
  | "authorization-header"
  | "bearer-value"
  | "webhook-url"
  | "apprise-url"
  | "ntfy-topic-url"
  | "ntfy-token"
  | "smtp-auth"
  | "secret-field"
  | "pem-private-key"
  | "url-credentials";

interface Shape {
  kind: ShapeKind;
  pattern: RegExp;
  holdsValue?: (match: RegExpExecArray, line: string) => boolean;
}

const B64URL = String.raw`A-Za-z0-9_\-`;
const PLACEHOLDER = /^(?:null|none|true|false|\*+|\[redacted\]|<redacted>|redacted)$/i;
const APPRISE_SCHEMES =
  "apprises?|discord|tgram|slack|pover|pbul|gchat|pagerduty|opsgenie|" +
  "mailtos?|ntfys?|pushover|gotifys?|matrixs?|msteams|jsons?|xmls?|forms?|signals?|rockets?|mmosts?|zulip|join|" +
  "ifttt|twilio";
const USER = String.raw`[^\s/?#@:"'<>]`;
const DISCORD_WEBHOOK = String.raw`discord(?:app)?\.com/api/(?:v\d+/)?webhooks/\d+/[A-Za-z0-9_-]`;
const SLACK_WEBHOOK = String.raw`hooks\.slack\.com/(?:services|workflows|triggers)/[A-Za-z0-9]`;
const TEAMS_WEBHOOK = String.raw`webhook\.office\.com/webhookb2/[A-Za-z0-9]`;
const SMTP_MECHANISMS = "PLAIN|LOGIN|XOAUTH2|OAUTHBEARER|CRAM-MD5";

function isRandom(value: string): boolean {
  return /[A-Z]/.test(value) && /[a-z]/.test(value) && /\d/.test(value);
}

function isBearerValue(match: RegExpExecArray, line: string): boolean {
  const before = line.slice(Math.max(0, match.index - 7), match.index);
  return isRandom(match[0]) && !/sha256[:-]$/i.test(before);
}

function isRealValue(value: string | undefined): boolean {
  return value !== undefined && value !== "" && !PLACEHOLDER.test(value);
}

function isSecretField(match: RegExpExecArray, line: string): boolean {
  const before = line.charAt(match.index - 1);
  const name = match.groups?.name ?? "";
  const atWordStart = !/[A-Za-z]/.test(before) || (/^[A-Z]/.test(name) && /[a-z0-9]/.test(before));
  return atWordStart && isRealValue(match.groups?.value);
}

function isUrlCredential(match: RegExpExecArray): boolean {
  const tokenUser = match.groups?.tokenUser;
  return match.groups?.password !== undefined || (tokenUser !== undefined && isRandom(tokenUser));
}

const SHAPES: readonly Shape[] = [
  // R-126: an `anima_agent_` token is still a machine secret.
  { kind: "daemon-token", pattern: new RegExp(`anima_(?:daemon|agent)_[${B64URL}]{43}(?![${B64URL}])`, "g") },
  { kind: "enrolment-code", pattern: new RegExp(`anima_enrol_[${B64URL}]{22}(?![${B64URL}])`, "g") },
  { kind: "api-key", pattern: new RegExp(`anima_[${B64URL}]{43}(?![${B64URL}])`, "g") },
  { kind: "browser-cookie", pattern: /__Host-anima=[^;\s"',]/g },
  {
    kind: "authorization-header",
    pattern: /authorization\\?["']?\s*[:=]\s*\\?["']?(?:bearer|basic)\s+[A-Za-z0-9._~+/=-]{8,}/gi,
  },
  {
    kind: "bearer-value",
    pattern: new RegExp(`(?<![${B64URL}+])[${B64URL}]{43}(?![${B64URL}+])`, "g"),
    holdsValue: isBearerValue,
  },
  {
    kind: "webhook-url",
    pattern: new RegExp(`${DISCORD_WEBHOOK}|${SLACK_WEBHOOK}|${TEAMS_WEBHOOK}`, "gi"),
  },
  {
    kind: "apprise-url",
    pattern: new RegExp(`(?<![A-Za-z0-9+.-])(?:${APPRISE_SCHEMES})://[^\\s"'<>]`, "gi"),
  },
  { kind: "ntfy-topic-url", pattern: /https?:\/\/[A-Za-z0-9.-]{0,253}ntfy[A-Za-z0-9.-]{0,253}(?::\d+)?\/[A-Za-z0-9_-]/gi },
  { kind: "ntfy-token", pattern: /(?<![A-Za-z0-9_])tk_[A-Za-z0-9]{29}(?![A-Za-z0-9_])/g },
  {
    kind: "smtp-auth",
    // The lookahead skips a server's all-capitals mechanism list (`250-AUTH PLAIN LOGIN`).
    pattern: new RegExp(
      String.raw`\bAUTH\s+(?:${SMTP_MECHANISMS})\s+(?=\S*[a-z+/=])[A-Za-z0-9+/]{4,}={0,2}(?![\w+/=-])`,
      "g",
    ),
  },
  {
    kind: "secret-field",
    pattern: /(?<name>password|passwd|passphrase|secret)\\?["']?\s*[:=][ \t]*\\?["']?(?<value>[^\s"',;&}\\]*)/gi,
    holdsValue: isSecretField,
  },
  { kind: "pem-private-key", pattern: /-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----/g },
  {
    kind: "url-credentials",
    pattern: new RegExp(
      `(?<![A-Za-z0-9+.-])[A-Za-z][A-Za-z0-9+.-]{0,31}://(?:${USER}*:(?<password>[^\\s/?#@"'<>]+)|(?<tokenUser>${USER}{20,}))@`,
      "g",
    ),
    holdsValue: isUrlCredential,
  },
];

export const SHAPE_KINDS: readonly ShapeKind[] = SHAPES.map((shape) => shape.kind);

function hasShape(shape: Shape, line: string): boolean {
  for (const match of line.matchAll(shape.pattern)) {
    if (shape.holdsValue === undefined || shape.holdsValue(match as RegExpExecArray, line)) {
      return true;
    }
  }
  return false;
}

export function shapeKinds(line: string): ShapeKind[] {
  return SHAPES.filter((shape) => hasShape(shape, line)).map((shape) => shape.kind);
}
