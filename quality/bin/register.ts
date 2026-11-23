import { existsSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { MOCKUPS, mockupTable, withMockupTable } from "../src/documents/mockup-records.ts";
import { QUEUE, queueRows, withQueue } from "../src/documents/queue.ts";
import { REGISTER, registerRows, withRows } from "../src/documents/register.ts";
import { settleGenerated } from "../src/merge/finish.ts";
import { listMeasurableFiles, readRepoFile, REPO_ROOT } from "../src/repository.ts";

// `--settle` first settles a merge that stopped only on generated files and the mockups.
if (process.argv.includes("--settle")) {
  const reason = settleGenerated(REPO_ROOT);
  if (reason !== undefined) {
    console.error(`register: the merge needs a person: ${reason}`);
    process.exit(1);
  }
}

const documents = [...new Set(listMeasurableFiles(REPO_ROOT))]
  .filter((path) => /^docs\/.*\.md$/.test(path))
  .sort()
  .map((path) => ({ path, text: readRepoFile(REPO_ROOT, path) }));

function existing(path: string): string {
  if (existsSync(join(REPO_ROOT, path))) return readRepoFile(REPO_ROOT, path);
  console.error(`register: ${path} does not exist; create it with the two marker comments of its generated part`);
  process.exit(1);
}

const written = withRows(existing(REGISTER), registerRows(documents));
if (written === undefined) {
  console.error(`register: ${REGISTER} has no generated list`);
  process.exit(1);
}
writeFileSync(join(REPO_ROOT, REGISTER), written);
console.log(`register: wrote ${REGISTER}`);

const queue = withQueue(existing(QUEUE), queueRows(documents));
if (queue === undefined) {
  console.error(`register: ${QUEUE} has no generated part`);
  process.exit(1);
}
writeFileSync(join(REPO_ROOT, QUEUE), queue);
console.log(`register: wrote ${QUEUE}`);

const mockups = withMockupTable(existing(MOCKUPS), mockupTable(documents));
if (mockups === undefined) {
  console.error(`register: ${MOCKUPS} has no generated table`);
  process.exit(1);
}
writeFileSync(join(REPO_ROOT, MOCKUPS), mockups);
console.log(`register: wrote ${MOCKUPS}`);
