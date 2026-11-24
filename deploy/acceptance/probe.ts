// A live probe of a running hub (SEC-001, SEC-002, API-003, INT-001 to INT-003, D-3, D-4, D-23, D-24, R-100):
//   bun deploy/acceptance/probe.ts https://hub.anima.ing [--key-file <path>] [--ca <pem file>]
// Prints `ok <check>` or `FAIL <check>: <reason>`, then a summary; a reason never quotes what the hub sent (A-2).
// Exit codes: 0 every check passed, 1 a check failed, 2 bad usage, an unusable key or CA file, or no hub answer.
// The key (a READ key) is read only from `--key-file`; every document sent with it is invalid, so nothing changes.
// Redirects are never followed, so the key never leaves the origin.
import { constants } from "node:fs";
import { open, type FileHandle } from "node:fs/promises";
import { parseArgs } from "node:util";
import { familiesVerdict, labelValuesVerdict, labelsVerdict } from "./exposition.ts";
import { ask, fakeCookie, hubOrigin, postGraphql, preflight, proxyConfigured, socketOutcome, type Answer, type Target } from "./hub-requests.ts";
import { parseExposition, type Family } from "./prometheus-text.ts";
import * as refusals from "./refusals.ts";
import { IMMUTABLE, NO_CACHE, cacheVerdict, fixedHeadersVerdict, immutableAssetPath, policyVerdict } from "./static-headers.ts";

const KEY_SHAPE = /^anima_[A-Za-z0-9_-]{43}$/;
const EXPOSITION_TYPE = "text/plain; version=0.0.4; charset=utf-8";
const USAGE = "usage: probe.ts <https://hub origin> [--key-file <path>] [--ca <pem file>]";
const RUN_LIMIT_MS = 120_000;
const NO_ANSWER = "no answer (network, TLS, timeout or too large)";

interface Check {
  name: string;
  reason: string | undefined;
}

type Verdict = (answer: Answer) => string | undefined;

class ProbeError extends Error {}

function check(name: string, answer: Answer | undefined, verdict: Verdict): Check {
  return { name, reason: answer === undefined ? NO_ANSWER : verdict(answer) };
}

function served(verdict: Verdict): Verdict {
  return (answer) => (answer.status === 200 ? verdict(answer) : `answered status ${String(answer.status)}, not 200`);
}

async function assetChecks(target: Target, index: Answer | undefined): Promise<Check[]> {
  const path = index === undefined ? undefined : immutableAssetPath(index.body);
  if (path === undefined) {
    const reason = index === undefined ? NO_ANSWER : "the page names no file under /_app/immutable/";
    return [{ name: "asset-cache", reason }, { name: "asset-headers", reason }];
  }
  const asset = await ask(target, path);
  return [
    check("asset-cache", asset, served((answer) => cacheVerdict(answer.headers, IMMUTABLE))),
    check("asset-headers", asset, served((answer) =>
      fixedHeadersVerdict(answer.headers) ?? policyVerdict(answer.headers, target.origin))),
  ];
}

async function corsClientCheck(target: Target): Promise<Check> {
  for (const origin of refusals.CLIENT_ORIGINS) {
    const answer = await preflight(target, origin);
    const reason = answer === undefined ? NO_ANSWER : refusals.clientPreflightVerdict(answer.headers, origin);
    if (reason !== undefined) {
      return { name: "cors-client-origins", reason };
    }
  }
  return { name: "cors-client-origins", reason: undefined };
}

async function* signedOutChecks(target: Target, health: Answer): AsyncGenerator<Check> {
  yield check("health", health, served((answer) => (answer.body.trim() === "ok" ? undefined : "the body is not ok")));
  const index = await ask(target, "/");
  yield check("index-csp", index, served((answer) => policyVerdict(answer.headers, target.origin)));
  yield check("index-headers", index, served((answer) => fixedHeadersVerdict(answer.headers)));
  yield check("index-cache", index, served((answer) => cacheVerdict(answer.headers, NO_CACHE)));
  yield* await assetChecks(target, index);
  const foreign = { origin: refusals.FOREIGN_ORIGIN, cookie: fakeCookie() };
  const signedOut = await postGraphql(target, refusals.IDENTITY_QUERY, foreign);
  yield check("graphql-foreign-cookie", signedOut, (answer) =>
    refusals.foreignCookieVerdict(answer.status, answer.headers, answer.body));
  yield { name: "socket-foreign-cookie", reason: refusals.socketVerdict(await socketOutcome(target)) };
  yield await corsClientCheck(target);
  const foreignPreflight = await preflight(target, refusals.FOREIGN_ORIGIN);
  yield check("cors-foreign-origin", foreignPreflight, (answer) => refusals.foreignPreflightVerdict(answer.headers));
  const metrics = await ask(target, "/metrics");
  yield check("metrics-without-token", metrics, (answer) => refusals.metricsRefusedVerdict(answer.status));
}

function expositionChecks(scraped: Answer | undefined): Check[] {
  const scrape = check("metrics-with-key", scraped, served((answer) =>
    answer.headers.get("content-type") === EXPOSITION_TYPE ? undefined : "the content-type is not the K7 text format"));
  // The text is read whenever the scrape answered 200, so a wrong content type fails only its own check.
  const parsed = scraped?.status === 200 ? parseExposition(scraped.body) : undefined;
  const first = parsed?.problems[0];
  const count = String(parsed?.problems.length);
  const problems = first === undefined ? undefined : `line ${String(first.line)}: ${first.problem} (${count} problems)`;
  const rule = (name: string, verdict: (families: Map<string, Family>) => string | undefined): Check => ({
    name,
    reason: parsed === undefined ? "no exposition to read" : verdict(parsed.families),
  });
  return [
    scrape,
    { name: "metrics-text", reason: parsed === undefined ? "no exposition to read" : problems },
    rule("metrics-families", familiesVerdict),
    rule("metrics-labels", labelsVerdict),
    rule("metrics-label-values", labelValuesVerdict),
  ];
}

async function* keyChecks(target: Target, key: string): AsyncGenerator<Check> {
  const authorization = { authorization: `Bearer ${key}` };
  yield* expositionChecks(await ask(target, "/metrics", { headers: authorization }));
  const me = await postGraphql(target, refusals.IDENTITY_QUERY, authorization);
  yield check("key-accepted", me, (answer) => refusals.keyAcceptedVerdict(answer.status, answer.body));
  const mutation = await postGraphql(target, refusals.ORDINARY_MUTATION, authorization);
  yield check("read-key-mutation", mutation, (answer) => refusals.readOnlyVerdict(answer.status, answer.body));
  for (const root of refusals.REFUSED_ROOTS) {
    const refused = await postGraphql(target, refusals.refusedRootDocument(root), authorization);
    yield check(`refused-root-${root}`, refused, (answer) => refusals.refusedRootVerdict(answer.status, answer.body));
  }
}

function errorCode(error: unknown): string {
  const code = (error as { code?: unknown } | null)?.code;
  return typeof code === "string" && /^[A-Z0-9_]+$/.test(code) ? code : "read error";
}

/** The size `fstat` gives is not trusted (S-C6). */
async function boundedText(handle: FileHandle, limit: number): Promise<string | undefined> {
  const buffer = Buffer.alloc(limit + 1);
  let length = 0;
  for (let read = 1; read > 0 && length <= limit; length += read) {
    ({ bytesRead: read } = await handle.read(buffer, length, limit + 1 - length, null));
  }
  return length > limit ? undefined : buffer.subarray(0, length).toString("utf8");
}

async function readSmallFile(path: string, what: "key file" | "CA file", limit: number): Promise<string> {
  const unreadable = `the ${what} is not a regular file of at most ${String(limit)} bytes`;
  try {
    // Non-blocking, so a FIFO in place of the file cannot hang the probe; the stat is of the opened file.
    const handle = await open(path, constants.O_RDONLY | constants.O_NONBLOCK);
    try {
      const stat = await handle.stat();
      if (!stat.isFile()) {
        throw new ProbeError(unreadable);
      }
      if (what === "key file" && process.platform !== "win32" && (stat.mode & 0o077) !== 0) {
        process.stderr.write("probe: warning: the key file can be read by group or other users; chmod 600 it\n");
      }
      const text = await boundedText(handle, limit);
      if (text === undefined) {
        throw new ProbeError(unreadable);
      }
      return text;
    } finally {
      await handle.close();
    }
  } catch (error) {
    throw error instanceof ProbeError ? error : new ProbeError(`the ${what} cannot be read (${errorCode(error)})`);
  }
}

/** Refused unless it has the `anima_` key shape, so no other file is sent. */
async function readKey(path: string): Promise<string> {
  const key = (await readSmallFile(path, "key file", 4096)).trim();
  if (!KEY_SHAPE.test(key)) {
    throw new ProbeError("the key file does not hold one Anima API key (anima_ and 43 base64url characters)");
  }
  return key;
}

async function readCa(path: string): Promise<string> {
  const pem = await readSmallFile(path, "CA file", 1024 * 1024);
  if (!pem.includes("-----BEGIN CERTIFICATE-----")) {
    throw new ProbeError("the CA file holds no PEM certificate");
  }
  return pem;
}

interface Request {
  target: Target;
  keyFile: string | undefined;
}

async function parseRequest(args: string[]): Promise<Request> {
  let parsed;
  try {
    const options = { "key-file": { type: "string" }, ca: { type: "string" } } as const;
    parsed = parseArgs({ args, options, allowPositionals: true, strict: true });
  } catch {
    // The parser's message quotes the argument, which may be a pasted key.
    throw new ProbeError(USAGE);
  }
  const origin = parsed.positionals.length === 1 ? hubOrigin(parsed.positionals[0] ?? "") : undefined;
  if (origin === undefined) {
    throw new ProbeError(`${USAGE}; the origin is https://host[:port], http only for a loopback host`);
  }
  const ca = parsed.values.ca === undefined ? undefined : await readCa(parsed.values.ca);
  return { target: { origin, ca }, keyFile: parsed.values["key-file"] };
}

async function report(groups: AsyncGenerator<Check>[]): Promise<{ total: number; failed: number }> {
  let total = 0;
  let failed = 0;
  for (const group of groups) {
    for await (const { name, reason } of group) {
      total += 1;
      failed += reason === undefined ? 0 : 1;
      process.stdout.write(reason === undefined ? `ok ${name}\n` : `FAIL ${name}: ${reason}\n`);
    }
  }
  return { total, failed };
}

async function main(args: string[]): Promise<number> {
  try {
    if (proxyConfigured(process.env)) {
      throw new ProbeError("a proxy variable (HTTP_PROXY, HTTPS_PROXY or ALL_PROXY) is set; unset it to probe the hub directly");
    }
    const request = await parseRequest(args);
    const key = request.keyFile === undefined ? undefined : await readKey(request.keyFile);
    const health = await ask(request.target, "/health");
    if (health === undefined) {
      throw new ProbeError("the hub cannot be reached (network, TLS or timeout)");
    }
    const groups = [signedOutChecks(request.target, health)];
    if (key !== undefined) {
      if (process.platform === "win32") process.stderr.write("probe: warning: verify the key file's Windows access permissions\n");
      groups.push(keyChecks(request.target, key));
    }
    const { total, failed } = await report(groups);
    const scope = key === undefined ? " (no --key-file: the key checks did not run)" : "";
    process.stdout.write(`${String(total)} checks, ${String(failed)} failed${scope}\n`);
    return failed === 0 ? 0 : 1;
  } catch (error) {
    // Only fixed text reaches stderr: a runtime message could quote an argument, a file or an answer (A-2).
    process.stderr.write(`probe: ${error instanceof ProbeError ? error.message : "unexpected failure"}\n`);
    return 2;
  }
}

if (import.meta.main) {
  process.stdout.on("error", () => {
    process.stderr.write("probe: the output closed\n");
    process.exit(2);
  });
  // One deadline for the whole run: a hub that answers each request just inside its timeout cannot hold it open.
  const deadline = setTimeout(() => {
    process.stderr.write(`probe: the run did not end within ${String(RUN_LIMIT_MS / 1000)} s\n`);
    process.exit(2);
  }, RUN_LIMIT_MS);
  process.exitCode = await main(process.argv.slice(2));
  clearTimeout(deadline);
}
