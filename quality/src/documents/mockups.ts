import { posix } from "node:path";
import { placeholders } from "../merge/numbers.ts";
import type { Violation } from "../report.ts";
import type { Document } from "./documents.ts";
import { isGenerated, isRecord, MOCKUPS, mockupTable, recordCells, withMockupTable } from "./mockup-records.ts";

export { MOCKUPS } from "./mockup-records.ts";
const LINK = /\]\(([^)\s#]+)\)/g;
const COMMIT = /`([0-9a-f]{7,40})`/;

export interface MockupRow {
  path: string;
  name: string;
  screens: string[];
  built: boolean;
  compared: string | undefined;
}

function targets(cell: string): string[] {
  return [...cell.matchAll(LINK)].map((match) => posix.normalize(posix.join(posix.dirname(MOCKUPS), match[1] ?? "")));
}

export function mockupRows(documents: Document[]): MockupRow[] {
  return documents
    .filter((document) => isRecord(document.path))
    .flatMap((document) => {
      const cells = recordCells(document.text);
      if (cells === undefined) return [];
      return [
        {
          path: document.path,
          name: cells.Mockup,
          screens: targets(cells.Screens),
          built: /^\W*built\b/i.test(cells.State),
          compared: COMMIT.exec(cells.Compared)?.[1],
        },
      ];
    });
}

function tableFaults(documents: Document[]): Violation[] {
  const index = documents.find((document) => document.path === MOCKUPS);
  if (index === undefined) return [];
  if (!isGenerated(index.text)) return [{ path: MOCKUPS, signal: "mockup-table-not-generated", detail: "the index has no generated table; run bun run quality:register" }];
  const records = documents.filter((document) => isRecord(document.path));
  const broken = records.filter((record) => recordCells(record.text) === undefined).map((record) => ({ path: record.path, signal: "mockup-record-without-mockup", detail: "the record has no line `- Mockup:`" }));
  const stale = withMockupTable(index.text, mockupTable(records)) !== index.text;
  return [...broken, ...(stale ? [{ path: MOCKUPS, signal: "mockup-table-stale", detail: "the table is not the one of docs/mockup/records/; run bun run quality:register" }] : [])];
}

/** PLN-019: a Built mockup names the commit it was compared at, and no layout file of its screens changed after it. */
export function checkMockups(documents: Document[], changedSince: (screen: string, commit: string) => number | undefined): Violation[] {
  const violations = tableFaults(documents);
  for (const row of mockupRows(documents)) {
    const commit = row.compared;
    if (commit === undefined) {
      if (row.built) violations.push({ path: row.path, signal: "mockup-not-compared", detail: row.name });
      continue;
    }
    if (row.built && row.screens.length === 0) violations.push({ path: row.path, signal: "mockup-without-screens", detail: row.name });
    for (const screen of row.screens) {
      const count = changedSince(screen, commit);
      if (count === undefined) violations.push({ path: row.path, signal: "mockup-commit-not-found", detail: `${commit} of ${row.name}` });
      else if (count > 0) {
        violations.push({ path: row.path, signal: "screen-changed-after-mockup", detail: `${screen} has ${count} layout file(s) that changed after ${commit}; compare ${row.name} again` });
      }
    }
  }
  return violations;
}

function isScreenLayout(path: string, screen: string): boolean {
  return path.startsWith(`${screen}/`) && (path.endsWith(".svelte") || path.endsWith(".css"));
}

/** PLN-019, PLN-021: the merge numbers a placeholder after the mockup was compared, so the mockup check would fail on the merge. */
export function placeholdersInScreens(records: Document[], paths: string[], read: (path: string) => string): Violation[] {
  const screens = mockupRows(records).flatMap((row) => row.screens);
  return paths
    .filter((path) => screens.some((screen) => isScreenLayout(path, screen)))
    .flatMap((path) =>
      placeholders([{ path, text: read(path) }]).map((placeholder) => ({
        path,
        signal: "number-placeholder-in-screen",
        detail: `${placeholder}: the merge would number it after the mockup was compared; name the plan row or a number that exists`,
      })),
    );
}
