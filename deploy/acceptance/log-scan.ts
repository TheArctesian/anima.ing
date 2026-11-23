// Scans a hub log for the never-log shapes of K7 (LOG-007, SEC-009); it never prints input text (A-3).
//   bash -o pipefail -c 'docker compose logs --no-color hub | bun deploy/acceptance/log-scan.ts [--needles <file>]'
//   bun deploy/acceptance/log-scan.ts [--needles <file>] <log file>
// Prints `line <n>: <kind>` per hit, then `scanned <n> lines, <m> hits`. The needles file holds one value per line.
// Exit codes: 0 no hit, 1 one or more hits, 2 bad usage, an empty or unreadable log or needles file, or closed output.
// Hub log fields are Rust Debug strings escaped again in JSON, so each line is also read unescaped and percent-decoded.
import { createReadStream } from "node:fs";
import { readFile } from "node:fs/promises";
import { parseArgs } from "node:util";
import { SHAPE_KINDS, shapeKinds, type ShapeKind } from "./leak-shapes.ts";

export type LeakKind = ShapeKind | "needle";

const ESC = String.fromCharCode(0x1b);
const BEL = String.fromCharCode(0x07);
// CSI and OSC sequences that `docker compose logs` may add.
const TERMINAL_CODES = new RegExp(`${ESC}\\[[0-9;?]*[ -/]*[@-~]|${ESC}\\][^${BEL}${ESC}]*(?:${BEL}|${ESC}\\\\)`, "g");
const ESCAPE = /\\(?:u([0-9A-Fa-f]{4})|u\{([0-9A-Fa-f]{1,6})\}|(["\\/'bfnrt0]))/g;
const ESCAPED: Readonly<Record<string, string>> = {
  '"': '"', "\\": "\\", "/": "/", "'": "'", b: "\b", f: "\f", n: "\n", r: "\r", t: "\t", 0: "\0",
};
const KIND_ORDER: readonly LeakKind[] = [...SHAPE_KINDS, "needle"];
const PERCENT = /%([0-9A-Fa-f]{2})/g;
const MAX_PASSES = 4;

export function plainLine(raw: string): string {
  return raw.replace(/\r$/, "").replace(TERMINAL_CODES, "");
}

function unescapeOnce(text: string): string {
  return text.replace(ESCAPE, (escape, unit?: string, point?: string, single?: string) => {
    const code = Number.parseInt(unit ?? point ?? "", 16);
    if (single !== undefined) {
      return ESCAPED[single] ?? escape;
    }
    return code <= 0x10ffff ? String.fromCodePoint(code) : escape;
  });
}

/** ASCII percent escapes only, so decoding never fails on a stray `%` or a broken UTF-8 sequence. */
function percentDecodeOnce(text: string): string {
  return text.replace(PERCENT, (escape, hex: string) => {
    const code = Number.parseInt(hex, 16);
    return code < 0x80 ? String.fromCharCode(code) : escape;
  });
}

function settle(text: string, needed: string, pass: (text: string) => string, views: string[]): string {
  let current = text;
  for (let round = 0; round < MAX_PASSES && current.includes(needed); round += 1) {
    const next = pass(current);
    if (next === current) {
      break;
    }
    views.push(next);
    current = next;
  }
  return current;
}

export function lineViews(line: string): string[] {
  const views = [line];
  const unescaped = settle(line, "\\", unescapeOnce, views);
  settle(unescaped, "%", percentDecodeOnce, views);
  return views;
}

const needleCache = new WeakMap<readonly string[], string[]>();

function urlForm(value: string): string | undefined {
  try {
    return new URL(value).href;
  } catch {
    return undefined;
  }
}

function needleForms(needles: readonly string[]): string[] {
  let forms = needleCache.get(needles);
  if (forms === undefined) {
    const urls = needles.map(urlForm).filter((href) => href !== undefined);
    forms = [...new Set([...needles, ...urls, ...urls.map((href) => href.replace(/\/$/, ""))])];
    needleCache.set(needles, forms);
  }
  return forms;
}

export function leakKinds(raw: string, needles: readonly string[]): LeakKind[] {
  const views = lineViews(plainLine(raw));
  const found = new Set<LeakKind>(views.flatMap(shapeKinds));
  const forms = needleForms(needles);
  if (views.some((view) => forms.some((form) => view.includes(form)))) {
    found.add("needle");
  }
  return KIND_ORDER.filter((kind) => found.has(kind));
}

export function parseNeedles(text: string): string[] {
  return text
    .split("\n")
    .map((value) => value.trim())
    .filter((value) => value !== "");
}

async function* streamLines(stream: AsyncIterable<string | Uint8Array>): AsyncGenerator<string> {
  const decoder = new TextDecoder();
  let rest = "";
  for await (const chunk of stream) {
    const text = typeof chunk === "string" ? chunk : decoder.decode(chunk, { stream: true });
    const end = text.lastIndexOf("\n");
    if (end === -1) {
      rest += text;
      continue;
    }
    yield* (rest + text.slice(0, end)).split("\n");
    rest = text.slice(end + 1);
  }
  rest += decoder.decode();
  if (rest !== "") {
    yield rest;
  }
}

class ScanError extends Error {}

interface Request {
  logPath: string | undefined;
  needlesPath: string | undefined;
}

function parseRequest(args: string[]): Request {
  let parsed;
  try {
    parsed = parseArgs({ args, options: { needles: { type: "string" } }, allowPositionals: true, strict: true });
  } catch {
    // The parser's message quotes the argument, which may be a secret value.
    throw new ScanError("usage: log-scan.ts [--needles <file>] [<log file> | -]");
  }
  if (parsed.positionals.length > 1) {
    throw new ScanError("usage: log-scan.ts [--needles <file>] [<log file> | -]");
  }
  const path = parsed.positionals[0];
  return { logPath: path === "-" ? undefined : path, needlesPath: parsed.values.needles };
}

async function readNeedles(path: string | undefined): Promise<string[]> {
  if (path === undefined) {
    return [];
  }
  let text: string;
  try {
    text = await readFile(path, "utf8");
  } catch (error) {
    throw new ScanError(`the needles file cannot be read (${errorCode(error)})`);
  }
  const needles = parseNeedles(text);
  if (needles.length === 0) {
    throw new ScanError("the needles file holds no values");
  }
  return needles;
}

function errorCode(error: unknown): string {
  const code = (error as { code?: unknown } | null)?.code;
  return typeof code === "string" && /^[A-Z0-9_]+$/.test(code) ? code : "read error";
}

function report(text: string): void {
  process.stdout.write(`${text}\n`);
}

async function scanStream(stream: AsyncIterable<string | Uint8Array>, needles: readonly string[]): Promise<number> {
  let number = 0;
  let hits = 0;
  try {
    for await (const raw of streamLines(stream)) {
      number += 1;
      for (const kind of leakKinds(raw, needles)) {
        hits += 1;
        report(`line ${String(number)}: ${kind}`);
      }
    }
  } catch (error) {
    throw new ScanError(`the log cannot be read after line ${String(number)} (${errorCode(error)})`);
  }
  if (number === 0) {
    throw new ScanError("the log is empty");
  }
  report(`scanned ${String(number)} lines, ${String(hits)} hits`);
  return hits;
}

export function needsLog(logPath: string | undefined, stdinIsTTY: boolean | undefined): boolean {
  return logPath === undefined && stdinIsTTY === true;
}

async function main(args: string[]): Promise<number> {
  try {
    const request = parseRequest(args);
    const needles = await readNeedles(request.needlesPath);
    if (needsLog(request.logPath, process.stdin.isTTY)) {
      throw new ScanError("no log on stdin: pipe the log in or give a file");
    }
    const stream = request.logPath === undefined ? process.stdin : createReadStream(request.logPath);
    return (await scanStream(stream, needles)) === 0 ? 0 : 1;
  } catch (error) {
    // Only fixed text reaches stderr: a runtime message could quote the log or a needle (A-3).
    const reason = error instanceof ScanError ? error.message : `unexpected failure (${errorCode(error)})`;
    process.stderr.write(`log-scan: ${reason}\n`);
    return 2;
  }
}

function stopOnClosedOutput(): void {
  process.stdout.on("error", (error) => {
    process.stderr.write(`log-scan: the output closed (${errorCode(error)})\n`);
    process.exit(2);
  });
}

if (import.meta.main) {
  stopOnClosedOutput();
  process.exitCode = await main(process.argv.slice(2));
}
