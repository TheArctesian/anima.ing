/** One record file per mockup, so branches that compare different mockups do not conflict (PLN-019). */
import { posix } from "node:path";
import type { Document } from "./documents.ts";
import { replaceBlock } from "./generated.ts";

export const MOCKUPS = "docs/mockup/README.md";
export const RECORDS = "docs/mockup/records/";
export const FIELDS = ["Mockup", "Owner", "Screens", "State", "Compared", "Differences"] as const;
export type Field = (typeof FIELDS)[number];
export type Cells = Record<Field, string>;

const START = "<!-- generated from docs/mockup/records/: bun run quality:register -->";
const END = "<!-- end of the generated table -->";
const LINK = /\]\(([^)\s]+)\)/g;
const FIELD = /^- (Mockup|Owner|Screens|State|Compared|Differences): ?(.*)$/;

export function isRecord(path: string): boolean {
  return path.startsWith(RECORDS) && path.endsWith(".md") && !path.slice(RECORDS.length).includes("/");
}

function rebased(text: string, from: string, to: string): string {
  return text.replace(LINK, (whole, target: string) => {
    if (/^[a-z][a-z0-9+.-]*:|^#/i.test(target)) return whole;
    return `](${posix.relative(to, posix.normalize(posix.join(from, target)))})`;
  });
}

export function recordPath(mockupCell: string): string | undefined {
  const target = /\]\(([^)\s#]+)\)/.exec(mockupCell)?.[1];
  if (target === undefined) return undefined;
  const name = target.replace(/\.html$/, "").replace(/\/index$/, "").replaceAll("/", "-");
  return `${RECORDS}${name}.md`;
}

export function recordCells(text: string): Cells | undefined {
  const found = new Map<string, string>();
  for (const line of text.split("\n")) {
    const match = FIELD.exec(line);
    if (match?.[1] !== undefined) found.set(match[1], rebased(match[2] ?? "", RECORDS, posix.dirname(MOCKUPS)));
  }
  if (!found.has("Mockup")) return undefined;
  return Object.fromEntries(FIELDS.map((field) => [field, found.get(field) ?? ""])) as Cells;
}

export function recordText(cells: Cells): string {
  const name = /\]\(([^)\s#]+)\)/.exec(cells.Mockup)?.[1] ?? cells.Mockup;
  const lines = FIELDS.map((field) => `- ${field}: ${rebased(cells[field], posix.dirname(MOCKUPS), RECORDS)}`);
  return [`# The record of the mockup ${name}`, "", "One row of the index in [README.md](../README.md) (PLN-019). `bun run quality:register` writes the row from this file.", "", ...lines, ""].join("\n");
}

export function tableRows(text: string): Cells[] {
  const start = text.indexOf("\n## Index");
  if (start < 0) return [];
  const rows = text.slice(start).split("\n").filter((line) => line.startsWith("|")).slice(2);
  return rows.map((row) => {
    const cells = row.split("|").slice(1, -1).map((cell) => cell.trim());
    return Object.fromEntries(FIELDS.map((field, at) => [field, cells[at] ?? ""])) as Cells;
  });
}

export function isGenerated(index: string): boolean {
  return index.includes(START);
}

export function mockupTable(records: Document[]): string {
  const rows = records
    .filter((record) => isRecord(record.path))
    .sort((a, b) => (a.path < b.path ? -1 : 1))
    .flatMap((record) => {
      const cells = recordCells(record.text);
      return cells === undefined ? [] : [`| ${FIELDS.map((field) => cells[field]).join(" | ")} |`];
    });
  return [START, "", `| ${FIELDS.join(" | ")} |`, `| ${FIELDS.map(() => "---").join(" | ")} |`, ...rows, "", END].join("\n");
}

export function withMockupTable(index: string, table: string): string | undefined {
  return replaceBlock(index, START, END, table);
}
